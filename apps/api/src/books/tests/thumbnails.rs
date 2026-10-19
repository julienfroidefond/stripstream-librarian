//! Tests for the thumbnail job-creation endpoints.
//!
//! The API never renders thumbnails itself — it only queues `index_jobs` rows
//! that the indexer picks up. These tests pin down which jobs are created.

use axum::http::StatusCode;

use super::*;
use crate::state::test_state;

async fn create_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
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

#[sqlx::test(migrations = "../../infra/migrations")]
async fn rebuild_creates_one_pending_job_for_the_requested_library(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "rebuild_one").await;
    let state = test_state(pool.clone());

    let response = start_thumbnails_rebuild(
        State(state),
        Some(Json(ThumbnailsRebuildRequest {
            library_id: Some(lib),
        })),
    )
    .await
    .unwrap();

    assert_eq!(response.0.r#type, "thumbnail_rebuild");
    assert_eq!(response.0.status, "pending");
    assert_eq!(response.0.library_id, Some(lib));

    let jobs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM index_jobs WHERE type = 'thumbnail_rebuild' AND library_id = $1",
    )
    .bind(lib)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(jobs, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn rebuild_creates_one_job_per_library_when_none_requested(pool: sqlx::PgPool) {
    let lib_a = create_library(&pool, "a_lib").await;
    let lib_b = create_library(&pool, "b_lib").await;
    let state = test_state(pool.clone());

    let response = start_thumbnails_rebuild(State(state), None).await.unwrap();

    // One job per existing library.
    let jobs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs WHERE type = 'thumbnail_rebuild'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(jobs, 2);

    // The returned job is the last library ordered by name ("b_lib" > "a_lib").
    assert_eq!(response.0.library_id, Some(lib_b));
    assert_ne!(lib_a, lib_b);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn rebuild_errors_when_no_library_exists(pool: sqlx::PgPool) {
    let state = test_state(pool.clone());

    let err = match start_thumbnails_rebuild(State(state), None).await {
        Ok(_) => panic!("expected an error when no library exists"),
        Err(err) => err,
    };

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(
        err.message.contains("No libraries"),
        "unexpected: {}",
        err.message
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn regenerate_creates_pending_job_for_the_requested_library(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "regen_one").await;
    let state = test_state(pool.clone());

    let response = start_thumbnails_regenerate(
        State(state),
        Some(Json(ThumbnailsRebuildRequest {
            library_id: Some(lib),
        })),
    )
    .await
    .unwrap();

    assert_eq!(response.0.r#type, "thumbnail_regenerate");
    assert_eq!(response.0.status, "pending");
    assert_eq!(response.0.library_id, Some(lib));

    let jobs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM index_jobs WHERE type = 'thumbnail_regenerate' AND library_id = $1",
    )
    .bind(lib)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(jobs, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn regenerate_creates_one_job_per_library_when_none_requested(pool: sqlx::PgPool) {
    create_library(&pool, "regen_a").await;
    create_library(&pool, "regen_b").await;
    let state = test_state(pool.clone());

    let _ = start_thumbnails_regenerate(State(state), None)
        .await
        .unwrap();

    let jobs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs WHERE type = 'thumbnail_regenerate'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(jobs, 2);
}
