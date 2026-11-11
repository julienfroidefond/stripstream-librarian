//! Integration tests for `GET /series/{series_id}/volumes`.

use axum::extract::{Path, State};
use sqlx::PgPool;
use uuid::Uuid;

use super::super::get_series_owned_volumes;
use super::{create_book_with_type, create_series, create_test_library};
use crate::state::test_state;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn includes_numbered_integrals_and_regular_volumes(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_test_library(&pool, "volumes_integral").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with_type(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with_type(&pool, library, series, "Vol 2", Some(2), "regular").await;
    // An integral of tome 2 covers that same volume.
    create_book_with_type(&pool, library, series, "INT 2", Some(2), "integral").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(
        dto.owned_volumes,
        vec![1, 2],
        "numbered integrals count as owned volumes"
    );
    assert!(!dto.has_unnumbered_integral);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn unnumbered_integral_sets_flag_and_keeps_regular_volumes(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_test_library(&pool, "volumes_int_null").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with_type(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with_type(&pool, library, series, "INT", None, "integral").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(
        dto.owned_volumes,
        vec![1],
        "regular volumes stay listed alongside the unnumbered integral"
    );
    assert!(
        dto.has_unnumbered_integral,
        "an unnumbered integral means the series is complete"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn excludes_hs_and_oneshot_volumes(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_test_library(&pool, "volumes_hs").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with_type(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with_type(&pool, library, series, "HS 5", Some(5), "hs").await;
    create_book_with_type(&pool, library, series, "One-shot 6", Some(6), "oneshot").await;
    create_book_with_type(&pool, library, series, "One-shot", None, "oneshot").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(
        dto.owned_volumes,
        vec![1],
        "HS and one-shots never count as owned"
    );
    assert!(!dto.has_unnumbered_integral);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn excludes_null_regular_volumes(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_test_library(&pool, "volumes_null").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with_type(&pool, library, series, "No volume", None, "regular").await;
    create_book_with_type(&pool, library, series, "Vol 3", Some(3), "regular").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(
        dto.owned_volumes,
        vec![3],
        "NULL regular volumes are excluded"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn excludes_volume_zero(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_test_library(&pool, "volumes_zero").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with_type(&pool, library, series, "Special", Some(0), "regular").await;
    create_book_with_type(&pool, library, series, "Vol 1", Some(1), "regular").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(
        dto.owned_volumes,
        vec![1],
        "volume 0 is a provider placeholder, not a tome"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn deduplicates_and_sorts_volumes(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_test_library(&pool, "volumes_sort").await;
    let series = create_series(&pool, library, "Series").await;

    create_book_with_type(&pool, library, series, "Vol 3", Some(3), "regular").await;
    create_book_with_type(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with_type(&pool, library, series, "Vol 3 dup", Some(3), "regular").await;
    create_book_with_type(&pool, library, series, "INT 1", Some(1), "integral").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert_eq!(dto.owned_volumes, vec![1, 3], "distinct and ascending");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn returns_empty_list_for_series_without_books(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_test_library(&pool, "volumes_empty").await;
    let series = create_series(&pool, library, "Empty Series").await;

    let axum::Json(dto) = get_series_owned_volumes(State(state), Path(series))
        .await
        .unwrap();

    assert!(dto.owned_volumes.is_empty());
    assert!(!dto.has_unnumbered_integral);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn returns_404_for_unknown_series(pool: PgPool) {
    let state = test_state(pool);
    let missing = Uuid::new_v4();

    let err = get_series_owned_volumes(State(state), Path(missing))
        .await
        .err()
        .expect("unknown series should return an error");

    assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(err.message, format!("series '{missing}' not found"));
}

/// Defence in depth: the admin route only accepts a UUID path segment.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn route_rejects_a_non_uuid_path(pool: PgPool) {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    let app = Router::new()
        .route("/series/{series_id}/volumes", get(get_series_owned_volumes))
        .with_state(test_state(pool));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/series/not-a-uuid/volumes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
