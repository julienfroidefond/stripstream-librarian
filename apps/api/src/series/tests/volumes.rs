//! Integration tests for `GET /series/{series_id}/volumes`.

use axum::extract::{Path, State};
use sqlx::PgPool;
use uuid::Uuid;

use super::super::get_series_owned_volumes;
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

async fn create_book_with(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    title: &str,
    volume: Option<i32>,
    volume_type: &str,
) {
    sqlx::query(
        "INSERT INTO books \
             (id, library_id, kind, title, series_id, volume, volume_type, authors, page_count) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, $4, $5, '{}', 10)",
    )
    .bind(library_id)
    .bind(title)
    .bind(series_id)
    .bind(volume)
    .bind(volume_type)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn returns_only_regular_volumes(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "volumes_regular").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    create_book_with(&pool, library, series, "HS 1", Some(3), "hs").await;
    create_book_with(&pool, library, series, "One-shot", Some(4), "oneshot").await;
    create_book_with(&pool, library, series, "Integral", Some(5), "integral").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(
        dto.owned_volumes,
        vec![1, 2],
        "only regular volumes count as owned"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn excludes_null_and_duplicate_volumes_and_sorts(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "volumes_sort").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with(&pool, library, series, "Vol 3", Some(3), "regular").await;
    create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with(&pool, library, series, "Vol 3 dup", Some(3), "regular").await;
    create_book_with(&pool, library, series, "No volume", None, "regular").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(
        dto.owned_volumes,
        vec![1, 3],
        "distinct, ascending, no NULLs"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn returns_empty_list_for_series_without_books(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "volumes_empty").await;
    let series = create_series(&pool, library, "Empty Series").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert!(dto.owned_volumes.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn returns_404_for_unknown_series(pool: PgPool) {
    let state = test_state(pool);

    let err = get_series_owned_volumes(State(state), Path(Uuid::new_v4()))
        .await
        .err()
        .expect("unknown series should return an error");

    assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
}
