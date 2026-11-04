//! Integration tests for `GET /admin/series/orphans`.
//!
//! Handlers run against the ephemeral Postgres provisioned by `#[sqlx::test]`,
//! so the `NOT EXISTS (books)` filter, the approved-link lateral join and the
//! available-download flag are exercised against real SQL.

use axum::extract::{Path, Query, State};
use axum::Json;
use sqlx::PgPool;
use uuid::Uuid;

use super::super::{delete_orphan_series, list_orphan_series, OrphanSeriesQuery};
use crate::state::test_state;

async fn create_library(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO libraries (id, name, root_path) \
         VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(format!("/libraries/{}-{}", name, Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_series(pool: &PgPool, library_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name) \
         VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_series_aged(pool: &PgPool, library_id: Uuid, name: &str, days: i32) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, created_at) \
         VALUES (gen_random_uuid(), $1, $2, NOW() - make_interval(days => $3)) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(days)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_book(pool: &PgPool, library_id: Uuid, series_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books (id, library_id, kind, title, series_id, volume, volume_type) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, 1, 'regular') RETURNING id",
    )
    .bind(library_id)
    .bind(title)
    .bind(series_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_link(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    provider: &str,
    status: &str,
    external_url: Option<&str>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO external_metadata_links \
             (library_id, series_id, provider, external_id, external_url, status) \
         VALUES ($1, $2, $3, 'ext-1', $4, $5) RETURNING id",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(provider)
    .bind(external_url)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_available_download(pool: &PgPool, library_id: Uuid, series_id: Uuid) {
    sqlx::query(
        "INSERT INTO available_downloads (library_id, series_id, missing_count) \
         VALUES ($1, $2, 1)",
    )
    .bind(library_id)
    .bind(series_id)
    .execute(pool)
    .await
    .unwrap();
}

fn query(page: Option<i64>, limit: Option<i64>) -> Query<OrphanSeriesQuery> {
    Query(OrphanSeriesQuery { page, limit })
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn lists_only_series_without_books(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "orphans-filter").await;

    let orphan = create_series(&pool, library, "Orphan").await;
    let with_book = create_series(&pool, library, "Owned").await;
    create_book(&pool, library, with_book, "Volume 1").await;

    let Json(page) = list_orphan_series(State(state), query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, orphan);
    assert_eq!(page.items[0].name, "Orphan");
    assert_eq!(page.items[0].library_id, library);
    assert_eq!(
        page.items[0].library_name.as_deref(),
        Some("orphans-filter")
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn exposes_approved_metadata_link_only(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "orphans-link").await;

    let linked = create_series(&pool, library, "Linked").await;
    create_link(
        &pool,
        library,
        linked,
        "senscritique",
        "approved",
        Some("https://www.senscritique.com/bd/linked"),
    )
    .await;

    let pending = create_series(&pool, library, "Pending").await;
    create_link(
        &pool,
        library,
        pending,
        "senscritique",
        "pending",
        Some("https://www.senscritique.com/bd/pending"),
    )
    .await;

    let Json(page) = list_orphan_series(State(state), query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 2);
    let linked_item = page.items.iter().find(|i| i.id == linked).unwrap();
    assert_eq!(
        linked_item.metadata_provider.as_deref(),
        Some("senscritique")
    );
    assert_eq!(
        linked_item.external_url.as_deref(),
        Some("https://www.senscritique.com/bd/linked")
    );

    let pending_item = page.items.iter().find(|i| i.id == pending).unwrap();
    assert!(pending_item.metadata_provider.is_none());
    assert!(pending_item.external_url.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn flags_series_with_wishlist_release(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "orphans-download").await;

    let wishlisted = create_series(&pool, library, "Wishlisted").await;
    create_available_download(&pool, library, wishlisted).await;
    let plain = create_series(&pool, library, "Plain").await;

    let Json(page) = list_orphan_series(State(state), query(None, None))
        .await
        .unwrap();

    let wishlisted_item = page.items.iter().find(|i| i.id == wishlisted).unwrap();
    assert!(wishlisted_item.has_wishlist_release);
    let plain_item = page.items.iter().find(|i| i.id == plain).unwrap();
    assert!(!plain_item.has_wishlist_release);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sorts_oldest_first_and_paginates(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "orphans-sort").await;

    let old = create_series_aged(&pool, library, "Old", 40).await;
    let middle = create_series_aged(&pool, library, "Middle", 10).await;
    let recent = create_series_aged(&pool, library, "Recent", 1).await;

    let Json(first) = list_orphan_series(State(state.clone()), query(Some(1), Some(2)))
        .await
        .unwrap();
    assert_eq!(first.total, 3);
    assert_eq!(first.page, 1);
    assert_eq!(first.limit, 2);
    let ids: Vec<Uuid> = first.items.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![old, middle]);

    let Json(second) = list_orphan_series(State(state), query(Some(2), Some(2)))
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].id, recent);

    let old_item = first.items.iter().find(|i| i.id == old).unwrap();
    assert!(
        (39..=41).contains(&old_item.age_days),
        "age_days should be ~40, got {}",
        old_item.age_days
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn deleting_an_orphan_archives_it_and_cascades_links(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "orphans-delete").await;

    let series_id = create_series(&pool, library, "Doomed").await;
    create_link(
        &pool,
        library,
        series_id,
        "senscritique",
        "approved",
        Some("https://www.senscritique.com/bd/doomed"),
    )
    .await;
    create_available_download(&pool, library, series_id).await;

    let _ = delete_orphan_series(State(state), Path(series_id))
        .await
        .unwrap();

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!exists, "series row must be deleted");

    let archived: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM archived_series WHERE id = $1)")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(archived, "series must be archived before deletion");

    let links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(links, 0, "metadata links must cascade");

    let downloads: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads WHERE series_id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(downloads, 0, "available downloads must cascade");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn ignores_approved_link_from_another_library(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "orphans-link-home").await;
    let other_library = create_library(&pool, "orphans-link-other").await;

    let series_id = create_series(&pool, library, "Scoped").await;
    create_link(
        &pool,
        other_library,
        series_id,
        "senscritique",
        "approved",
        Some("https://www.senscritique.com/bd/other-library"),
    )
    .await;

    let Json(page) = list_orphan_series(State(state), query(None, None))
        .await
        .unwrap();

    let item = page.items.iter().find(|i| i.id == series_id).unwrap();
    assert!(item.metadata_provider.is_none());
    assert!(item.external_url.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_orphan_series_rejects_series_that_gained_a_book(pool: PgPool) {
    use axum::http::StatusCode;

    let state = test_state(pool.clone());
    let library = create_library(&pool, "orphans-delete-stale").await;

    let series_id = create_series(&pool, library, "No longer orphan").await;
    create_book(&pool, library, series_id, "Volume 1").await;

    let error = delete_orphan_series(State(state), Path(series_id))
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::CONFLICT);

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(exists, "a non-orphan series must never be deleted");

    let books: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(books, 1, "the book must be preserved");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_orphan_series_returns_not_found_for_unknown_id(pool: PgPool) {
    use axum::http::StatusCode;

    let state = test_state(pool.clone());

    let error = delete_orphan_series(State(state), Path(Uuid::new_v4()))
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::NOT_FOUND);
}
