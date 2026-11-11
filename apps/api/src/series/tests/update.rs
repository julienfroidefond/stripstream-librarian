//! Tests for lock sanitization on `PATCH /series/{series_id}`.
//!
//! The metadata policy only reads a fixed set of lock keys as strict booleans.
//! Any other key (locks left by older UI versions) must never be persisted.

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;
use uuid::Uuid;

use crate::series::update::{update_series_by_id, UpdateSeriesRequest};
use crate::state::test_state;

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

async fn create_test_series(pool: &sqlx::PgPool, library_id: Uuid, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, created_at, updated_at) \
         VALUES (gen_random_uuid(), $1, $2, NOW(), NOW()) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn request(locked_fields: Option<serde_json::Value>) -> UpdateSeriesRequest {
    UpdateSeriesRequest {
        new_name: "Berserk".to_string(),
        authors: vec![],
        genres: vec![],
        author: None,
        language: None,
        description: Some("A dark fantasy".to_string()),
        publishers: vec![],
        start_year: None,
        total_volumes: None,
        status: None,
        locked_fields,
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_series_sanitizes_locked_fields(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_test_library(&pool, "series_locks").await;
    let series_id = create_test_series(&pool, library_id, "Berserk").await;

    let locked_fields = json!({
        "description": true,
        "cover_url": true,
        "authors": "true",
        "publishers": 1,
        "genres": true,
        "title": true,
        "series": true,
        "status": false,
    });

    let _ = update_series_by_id(
        State(state),
        None,
        Path(series_id),
        Json(request(Some(locked_fields))),
    )
    .await
    .unwrap();

    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT locked_fields FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        stored,
        json!({"description": true, "cover_url": true}),
        "unknown keys, non-booleans and false values are dropped before the write"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_series_without_locked_fields_clears_them(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_test_library(&pool, "series_locks_absent").await;
    let series_id = create_test_series(&pool, library_id, "Berserk").await;

    sqlx::query("UPDATE series SET locked_fields = $1 WHERE id = $2")
        .bind(json!({"description": true}))
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

    let _ = update_series_by_id(State(state), None, Path(series_id), Json(request(None)))
        .await
        .unwrap();

    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT locked_fields FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        stored,
        json!({}),
        "the existing PATCH contract treats an absent locked_fields as an empty map"
    );
}
