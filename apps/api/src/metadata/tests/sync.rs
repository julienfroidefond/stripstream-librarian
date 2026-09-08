use sqlx::Row;
use uuid::Uuid;

use super::super::shared_sync::{self, SeriesFields};

async fn create_test_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(format!("/libraries/{name}"))
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn create_series(pool: &sqlx::PgPool, lib_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO series (id, library_id, name) VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(lib_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Run the same INSERT...ON CONFLICT query used in sync_series_metadata,
/// testing the SQL logic directly without requiring a full AppState.
#[allow(clippy::too_many_arguments)]
async fn run_sync_sql(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
    description: Option<&str>,
    publishers: &[String],
    start_year: Option<i32>,
    total_volumes: Option<i32>,
    status: Option<&str>,
    authors: &[String],
    genres: &[String],
    cover_url: Option<&str>,
) {
    sqlx::query(
        r#"
        INSERT INTO series (id, library_id, name, description, publishers, start_year, total_volumes, status, authors, genres, cover_url, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
        ON CONFLICT (library_id, name)
        DO UPDATE SET
            description = CASE
                WHEN (series.locked_fields->>'description')::boolean IS TRUE THEN series.description
                ELSE COALESCE(NULLIF(EXCLUDED.description, ''), series.description)
            END,
            publishers = CASE
                WHEN (series.locked_fields->>'publishers')::boolean IS TRUE THEN series.publishers
                WHEN array_length(EXCLUDED.publishers, 1) > 0 THEN EXCLUDED.publishers
                ELSE series.publishers
            END,
            start_year = CASE
                WHEN (series.locked_fields->>'start_year')::boolean IS TRUE THEN series.start_year
                ELSE COALESCE(EXCLUDED.start_year, series.start_year)
            END,
            total_volumes = CASE
                WHEN (series.locked_fields->>'total_volumes')::boolean IS TRUE THEN series.total_volumes
                ELSE COALESCE(EXCLUDED.total_volumes, series.total_volumes)
            END,
            status = CASE
                WHEN (series.locked_fields->>'status')::boolean IS TRUE THEN series.status
                ELSE COALESCE(EXCLUDED.status, series.status)
            END,
            authors = CASE
                WHEN (series.locked_fields->>'authors')::boolean IS TRUE THEN series.authors
                WHEN array_length(EXCLUDED.authors, 1) > 0 THEN EXCLUDED.authors
                ELSE series.authors
            END,
            genres = CASE
                WHEN array_length(EXCLUDED.genres, 1) > 0 THEN EXCLUDED.genres
                ELSE series.genres
            END,
            cover_url = COALESCE(NULLIF(EXCLUDED.cover_url, ''), series.cover_url),
            updated_at = NOW()
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .bind(description)
    .bind(publishers)
    .bind(start_year)
    .bind(total_volumes)
    .bind(status)
    .bind(authors)
    .bind(genres)
    .bind(cover_url)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_series_metadata_sets_cover_url(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_cover").await;
    let series_id = create_series(&pool, lib_id, "Cover Test").await;

    // Sync with cover_url in metadata
    run_sync_sql(
        &pool,
        lib_id,
        "Cover Test",
        Some("A description"),
        &[],
        None,
        None,
        None,
        &[],
        &[],
        Some("https://example.com/cover.jpg"),
    )
    .await;

    let row = sqlx::query("SELECT cover_url FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cover_url: Option<String> = row.get("cover_url");
    assert_eq!(
        cover_url,
        Some("https://example.com/cover.jpg".to_string()),
        "cover_url should be set from metadata sync"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_series_metadata_empty_cover_url_preserved(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_cover_empty").await;
    let series_id = create_series(&pool, lib_id, "Cover Preserve").await;

    // Set an existing cover_url
    sqlx::query("UPDATE series SET cover_url = $1 WHERE id = $2")
        .bind("https://example.com/old_cover.jpg")
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    // Sync with empty cover_url — NULLIF('', '') => NULL, COALESCE(NULL, old) => old
    run_sync_sql(
        &pool,
        lib_id,
        "Cover Preserve",
        None,
        &[],
        None,
        None,
        None,
        &[],
        &[],
        Some(""),
    )
    .await;

    let row = sqlx::query("SELECT cover_url FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cover_url: Option<String> = row.get("cover_url");
    assert_eq!(
        cover_url,
        Some("https://example.com/old_cover.jpg".to_string()),
        "existing cover_url should be preserved when sync provides empty string"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_series_metadata_null_cover_url_preserved(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_cover_null").await;
    let series_id = create_series(&pool, lib_id, "Cover Null").await;

    // Set an existing cover_url
    sqlx::query("UPDATE series SET cover_url = $1 WHERE id = $2")
        .bind("https://example.com/original.jpg")
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    // Sync with NULL cover_url — COALESCE(NULL, old) => old
    run_sync_sql(
        &pool,
        lib_id,
        "Cover Null",
        None,
        &[],
        None,
        None,
        None,
        &[],
        &[],
        None,
    )
    .await;

    let row = sqlx::query("SELECT cover_url FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let cover_url: Option<String> = row.get("cover_url");
    assert_eq!(
        cover_url,
        Some("https://example.com/original.jpg".to_string()),
        "existing cover_url should be preserved when sync provides NULL"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_series_metadata_overwrites_unlocked_description(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_description_refresh").await;
    let series_id = create_series(&pool, lib_id, "Description Refresh").await;
    sqlx::query("UPDATE series SET description = $1 WHERE id = $2")
        .bind("Previous description")
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    shared_sync::upsert_series_metadata(
        &pool,
        lib_id,
        "Description Refresh",
        &SeriesFields {
            description: Some("Refreshed description".to_string()),
            authors: vec![],
            publishers: vec![],
            start_year: None,
            total_volumes: None,
            status: None,
            genres: vec![],
            cover_url: None,
        },
    )
    .await
    .unwrap();

    let description: Option<String> =
        sqlx::query_scalar("SELECT description FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(description.as_deref(), Some("Refreshed description"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn sync_series_metadata_preserves_locked_description(pool: sqlx::PgPool) {
    let lib_id = create_test_library(&pool, "sync_description_locked").await;
    let series_id = create_series(&pool, lib_id, "Description Locked").await;
    sqlx::query("UPDATE series SET description = $1, locked_fields = $2 WHERE id = $3")
        .bind("Manual description")
        .bind(serde_json::json!({ "description": true }))
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    shared_sync::upsert_series_metadata(
        &pool,
        lib_id,
        "Description Locked",
        &SeriesFields {
            description: Some("Provider description".to_string()),
            authors: vec![],
            publishers: vec![],
            start_year: None,
            total_volumes: None,
            status: None,
            genres: vec![],
            cover_url: None,
        },
    )
    .await
    .unwrap();

    let description: Option<String> =
        sqlx::query_scalar("SELECT description FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(description.as_deref(), Some("Manual description"));
}
