//! Tests for the destructive `POST /series/{target_id}/merge` handler.
//!
//! Unlike the low-level SQL snippets exercised in `tests/mod.rs`, these tests
//! drive the real `merge_series` handler so the guard clauses, the
//! provider-conflict de-duplication and the source deletion stay covered.

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::series::merge::{merge_series, MergeSeriesRequest};
use crate::state::test_state;

use super::{create_book, create_series, create_test_library};

/// Invoke the handler and return `(books_moved, metadata_moved, downloads_moved)`.
async fn merge(
    pool: &sqlx::PgPool,
    target_id: Uuid,
    source_id: Uuid,
) -> Result<(u64, u64, u64), ApiError> {
    let state = test_state(pool.clone());
    let response = merge_series(
        State(state),
        None::<Extension<AuthUser>>,
        Path(target_id),
        Json(MergeSeriesRequest { source_id }),
    )
    .await?;

    let body = response.0;
    Ok((body.books_moved, body.metadata_moved, body.downloads_moved))
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_series_rejects_merging_into_itself(pool: sqlx::PgPool) {
    let lib = create_test_library(&pool, "merge_self").await;
    let series = create_series(&pool, lib, "Solo").await;

    let err = merge(&pool, series, series).await.unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(
        err.message.contains("itself"),
        "unexpected message: {}",
        err.message
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_series_rejects_cross_library_merge(pool: sqlx::PgPool) {
    let lib_a = create_test_library(&pool, "merge_lib_a").await;
    let lib_b = create_test_library(&pool, "merge_lib_b").await;
    let target = create_series(&pool, lib_a, "Target").await;
    let source = create_series(&pool, lib_b, "Source").await;

    let err = merge(&pool, target, source).await.unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(
        err.message.contains("different libraries"),
        "unexpected message: {}",
        err.message
    );

    // Nothing must have been moved or deleted.
    let source_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        source_exists,
        "a rejected merge must leave the source intact"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_series_returns_not_found_for_unknown_source(pool: sqlx::PgPool) {
    let lib = create_test_library(&pool, "merge_unknown").await;
    let target = create_series(&pool, lib, "Target").await;

    let err = merge(&pool, target, Uuid::new_v4()).await.unwrap_err();
    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_series_moves_everything_and_deletes_source(pool: sqlx::PgPool) {
    let lib = create_test_library(&pool, "merge_full").await;
    let target = create_series(&pool, lib, "Target").await;
    let source = create_series(&pool, lib, "Source").await;

    create_book(&pool, lib, target, "Kept").await;
    create_book(&pool, lib, source, "Moved 1").await;
    create_book(&pool, lib, source, "Moved 2").await;

    // Metadata: the target already owns `google_books`; the source brings both
    // `google_books` (duplicate) and `comicvine` (new).
    for (series_id, provider) in [
        (target, "google_books"),
        (source, "google_books"),
        (source, "comicvine"),
    ] {
        sqlx::query(
            "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
             VALUES ($1, $2, $3, 'ext')",
        )
        .bind(lib)
        .bind(series_id)
        .bind(provider)
        .execute(&pool)
        .await
        .unwrap();
    }

    // A download and an AniList link only on the source side.
    sqlx::query(
        "INSERT INTO available_downloads (library_id, series_id, missing_count) VALUES ($1, $2, 3)",
    )
    .bind(lib)
    .bind(source)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO anilist_series_links (library_id, series_id, anilist_id) VALUES ($1, $2, 42)",
    )
    .bind(lib)
    .bind(source)
    .execute(&pool)
    .await
    .unwrap();

    let (books_moved, metadata_moved, downloads_moved) =
        merge(&pool, target, source).await.unwrap();
    assert_eq!(books_moved, 2);
    assert_eq!(metadata_moved, 1, "only comicvine is new to the target");
    assert_eq!(downloads_moved, 1);

    // The source series is gone.
    let source_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!source_exists, "the source series must be deleted");

    // All three books now live under the target.
    let target_books: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = $1")
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(target_books, 3);

    // Target keeps its own google_books link and gains comicvine; nothing remains
    // attached to the (now deleted) source.
    let providers: Vec<String> = sqlx::query_scalar(
        "SELECT provider FROM external_metadata_links WHERE series_id = $1 ORDER BY provider",
    )
    .bind(target)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        providers,
        vec!["comicvine".to_string(), "google_books".to_string()]
    );

    let leftover_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(leftover_links, 0);

    // The download moved and stayed unique per series.
    let download_owner: Option<Uuid> =
        sqlx::query_scalar("SELECT series_id FROM available_downloads WHERE library_id = $1")
            .bind(lib)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(download_owner, Some(target));

    // The AniList link moved too.
    let anilist_owner: Option<Uuid> =
        sqlx::query_scalar("SELECT series_id FROM anilist_series_links WHERE anilist_id = 42")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(anilist_owner, Some(target));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_series_keeps_target_metadata_on_provider_conflict(pool: sqlx::PgPool) {
    let lib = create_test_library(&pool, "merge_conflict").await;
    let target = create_series(&pool, lib, "Target").await;
    let source = create_series(&pool, lib, "Source").await;

    for (series_id, external_id) in [(target, "target_ext"), (source, "source_ext")] {
        sqlx::query(
            "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
             VALUES ($1, $2, 'senscritique', $3)",
        )
        .bind(lib)
        .bind(series_id)
        .bind(external_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (_books, metadata_moved, _downloads) = merge(&pool, target, source).await.unwrap();
    assert_eq!(metadata_moved, 0, "a duplicate provider must not be moved");

    let external_id: String = sqlx::query_scalar(
        "SELECT external_id FROM external_metadata_links \
         WHERE series_id = $1 AND provider = 'senscritique'",
    )
    .bind(target)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        external_id, "target_ext",
        "the target's own link must be preserved"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn merge_series_keeps_target_download_when_both_have_one(pool: sqlx::PgPool) {
    let lib = create_test_library(&pool, "merge_dl").await;
    let target = create_series(&pool, lib, "Target").await;
    let source = create_series(&pool, lib, "Source").await;

    for (series_id, missing_count) in [(target, 1), (source, 2)] {
        sqlx::query(
            "INSERT INTO available_downloads (library_id, series_id, missing_count) \
             VALUES ($1, $2, $3)",
        )
        .bind(lib)
        .bind(series_id)
        .bind(missing_count)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (_books, _metadata, downloads_moved) = merge(&pool, target, source).await.unwrap();
    assert_eq!(
        downloads_moved, 0,
        "the target already had a download row, so nothing is moved"
    );

    let (owner, missing_count): (Uuid, i32) = sqlx::query_as(
        "SELECT series_id, missing_count FROM available_downloads WHERE library_id = $1",
    )
    .bind(lib)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, target);
    assert_eq!(
        missing_count, 1,
        "the target's own download must be preserved"
    );
}
