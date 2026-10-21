//! Tests for the reading-list handlers.
//!
//! Handlers are called directly with the shared `test_state` helper, backed by
//! an ephemeral Postgres provisioned by `#[sqlx::test]`. This exercises the real SQL
//! (positions, cascade, per-user progress, genre restrictions) without starting an
//! HTTP server.

use axum::http::StatusCode;
use sqlx::PgPool;

use super::*;
use crate::state::test_state;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------
/// `unwrap_err` requires `T: Debug`, which the response DTOs do not implement.
fn expect_err<T>(result: Result<T, ApiError>) -> ApiError {
    match result {
        Ok(_) => panic!("expected an error result"),
        Err(err) => err,
    }
}

fn auth_user(user_id: Uuid) -> Option<Extension<AuthUser>> {
    Some(Extension(AuthUser { user_id }))
}

async fn create_user(pool: &PgPool, username: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn create_library(pool: &PgPool, name: &str) -> Uuid {
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

async fn create_series(pool: &PgPool, library_id: Uuid, name: &str, genres: &[&str]) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, genres) VALUES (gen_random_uuid(), $1, $2, $3) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(genres)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_book(pool: &PgPool, library_id: Uuid, series_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books (id, library_id, kind, title, series_id, volume, volume_type)
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, 1, 'regular') RETURNING id",
    )
    .bind(library_id)
    .bind(title)
    .bind(series_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn mark_read(pool: &PgPool, book_id: Uuid, user_id: Uuid) {
    sqlx::query(
        "INSERT INTO book_reading_progress (book_id, user_id, status) VALUES ($1, $2, 'read')
         ON CONFLICT (book_id, user_id) DO UPDATE SET status = 'read'",
    )
    .bind(book_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_list(state: &AppState, name: &str) -> Uuid {
    create_list_with(state, name, None).await
}

async fn create_list_with(state: &AppState, name: &str, description: Option<&str>) -> Uuid {
    let Json(dto) = create_reading_list(
        State(state.clone()),
        Json(CreateReadingListRequest {
            name: name.to_string(),
            description: description.map(str::to_string),
        }),
    )
    .await
    .unwrap();
    dto.id
}

async fn add(state: &AppState, list_id: Uuid, series_id: Uuid) -> Result<StatusCode, ApiError> {
    add_series(
        State(state.clone()),
        Path(list_id),
        Json(AddSeriesRequest { series_id }),
    )
    .await
}

async fn position_of(pool: &PgPool, list_id: Uuid, series_id: Uuid) -> Option<i32> {
    sqlx::query_scalar(
        "SELECT position FROM reading_list_items WHERE list_id = $1 AND series_id = $2",
    )
    .bind(list_id)
    .bind(series_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn item_count(pool: &PgPool, list_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM reading_list_items WHERE list_id = $1")
        .bind(list_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn positions_of(pool: &PgPool, list_id: Uuid) -> Vec<i32> {
    sqlx::query_scalar(
        "SELECT position FROM reading_list_items WHERE list_id = $1 ORDER BY position",
    )
    .bind(list_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// create / update / delete
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_reading_list_rejects_blank_name(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(
        create_reading_list(
            State(state),
            Json(CreateReadingListRequest {
                name: "   ".to_string(),
                description: None,
            }),
        )
        .await,
    );

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn create_reading_list_trims_name_and_defaults_counts(pool: PgPool) {
    let state = test_state(pool);

    let Json(dto) = create_reading_list(
        State(state),
        Json(CreateReadingListRequest {
            name: "  Favorites  ".to_string(),
            description: Some("My favorites".to_string()),
        }),
    )
    .await
    .unwrap();

    assert_eq!(dto.name, "Favorites");
    assert_eq!(dto.description.as_deref(), Some("My favorites"));
    assert_eq!(dto.series_count, 0);
    assert_eq!(dto.book_count, 0);
    assert_eq!(dto.books_read_count, 0);
    assert!(dto.preview_covers.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_reading_list_returns_not_found_for_unknown_id(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(
        update_reading_list(
            State(state),
            Path(Uuid::new_v4()),
            Json(UpdateReadingListRequest {
                name: Some("renamed".to_string()),
                description: None,
            }),
        )
        .await,
    );

    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_reading_list_updates_name_and_description(pool: PgPool) {
    let state = test_state(pool.clone());
    let list_id = create_list(&state, "Original").await;

    let Json(dto) = update_reading_list(
        State(state),
        Path(list_id),
        Json(UpdateReadingListRequest {
            name: Some("  Renamed  ".to_string()),
            description: Some(Some("Updated".to_string())),
        }),
    )
    .await
    .unwrap();

    assert_eq!(dto.id, list_id);
    assert_eq!(dto.name, "Renamed");
    assert_eq!(dto.description.as_deref(), Some("Updated"));
    assert_eq!(dto.series_count, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_reading_list_clears_description_on_explicit_null(pool: PgPool) {
    let state = test_state(pool.clone());
    let list_id = create_list_with(&state, "Keep me", Some("keep-me-description")).await;

    let Json(dto) = update_reading_list(
        State(state),
        Path(list_id),
        Json(UpdateReadingListRequest {
            name: None,
            description: Some(None),
        }),
    )
    .await
    .unwrap();

    assert_eq!(dto.description, None);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_reading_list_keeps_description_when_omitted(pool: PgPool) {
    let state = test_state(pool.clone());
    let list_id = create_list_with(&state, "Keep me", Some("keep-me-description")).await;

    let Json(dto) = update_reading_list(
        State(state),
        Path(list_id),
        Json(UpdateReadingListRequest {
            name: None,
            description: None,
        }),
    )
    .await
    .unwrap();

    assert_eq!(dto.description.as_deref(), Some("keep-me-description"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_reading_list_returns_not_found_for_unknown_id(pool: PgPool) {
    let state = test_state(pool);

    let err = delete_reading_list(State(state), Path(Uuid::new_v4()))
        .await
        .unwrap_err();

    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_reading_list_cascades_its_items(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "delete-cascade").await;
    let series_id = create_series(&pool, library_id, "Cascading", &[]).await;
    let list_id = create_list(&state, "Cascade").await;
    add(&state, list_id, series_id).await.unwrap();

    let status = delete_reading_list(State(state), Path(list_id))
        .await
        .unwrap();

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(item_count(&pool, list_id).await, 0);
}

// ---------------------------------------------------------------------------
// add / remove / reorder series
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn add_series_returns_not_found_when_list_missing(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "add-missing-list").await;
    let series_id = create_series(&pool, library_id, "Orphan", &[]).await;

    let err = add(&state, Uuid::new_v4(), series_id).await.unwrap_err();

    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn add_series_appends_with_incrementing_position(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "append").await;
    let first = create_series(&pool, library_id, "First", &[]).await;
    let second = create_series(&pool, library_id, "Second", &[]).await;
    let list_id = create_list(&state, "Append").await;

    add(&state, list_id, first).await.unwrap();
    add(&state, list_id, second).await.unwrap();

    assert_eq!(position_of(&pool, list_id, first).await, Some(0));
    assert_eq!(position_of(&pool, list_id, second).await, Some(1));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn add_series_rejects_duplicates(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "duplicate").await;
    let series_id = create_series(&pool, library_id, "Dup", &[]).await;
    let list_id = create_list(&state, "Dup list").await;

    add(&state, list_id, series_id).await.unwrap();
    let err = add(&state, list_id, series_id).await.unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(item_count(&pool, list_id).await, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn remove_series_returns_not_found_when_absent(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "remove-absent").await;
    let series_id = create_series(&pool, library_id, "Absent", &[]).await;
    let list_id = create_list(&state, "Absent list").await;

    let err = remove_series(State(state), Path((list_id, series_id)))
        .await
        .unwrap_err();

    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn remove_series_deletes_item(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "remove").await;
    let series_id = create_series(&pool, library_id, "Removable", &[]).await;
    let list_id = create_list(&state, "Remove list").await;
    add(&state, list_id, series_id).await.unwrap();

    let status = remove_series(State(state), Path((list_id, series_id)))
        .await
        .unwrap();

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(position_of(&pool, list_id, series_id).await, None);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn remove_series_compacts_remaining_positions(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "remove-gap").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let c = create_series(&pool, library_id, "C", &[]).await;
    let list_id = create_list(&state, "Remove gap").await;
    for series_id in [a, b, c] {
        add(&state, list_id, series_id).await.unwrap();
    }

    remove_series(State(state), Path((list_id, b)))
        .await
        .unwrap();

    assert_eq!(positions_of(&pool, list_id).await, vec![0, 1]);
    assert_eq!(position_of(&pool, list_id, a).await, Some(0));
    assert_eq!(position_of(&pool, list_id, c).await, Some(1));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn add_series_after_a_gap_keeps_positions_dense(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "add-gap").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let c = create_series(&pool, library_id, "C", &[]).await;
    let appended = create_series(&pool, library_id, "Appended", &[]).await;
    let list_id = create_list(&state, "Add gap").await;
    for series_id in [a, b, c] {
        add(&state, list_id, series_id).await.unwrap();
    }

    // Simulate a legacy list with a hole at position 1 (b) left by old code.
    sqlx::query("DELETE FROM reading_list_items WHERE list_id = $1 AND series_id = $2")
        .bind(list_id)
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(positions_of(&pool, list_id).await, vec![0, 2]);

    add(&state, list_id, appended).await.unwrap();

    assert_eq!(positions_of(&pool, list_id).await, vec![0, 1, 2]);
    assert_eq!(position_of(&pool, list_id, c).await, Some(1));
    assert_eq!(position_of(&pool, list_id, appended).await, Some(2));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn database_rejects_duplicate_positions(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "duplicate-position").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let list_id = create_list(&state, "Duplicate position").await;
    add(&state, list_id, a).await.unwrap();

    let result = sqlx::query(
        "INSERT INTO reading_list_items (list_id, series_id, position) VALUES ($1, $2, 0)",
    )
    .bind(list_id)
    .bind(b)
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "duplicate (list_id, position) must be rejected"
    );
    assert_eq!(item_count(&pool, list_id).await, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reorder_series_returns_not_found_when_list_missing(pool: PgPool) {
    let state = test_state(pool);

    let err = reorder_series(
        State(state),
        Path(Uuid::new_v4()),
        Json(ReorderSeriesRequest {
            series_ids: vec![Uuid::new_v4()],
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reorder_series_rewrites_positions(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "reorder").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let c = create_series(&pool, library_id, "C", &[]).await;
    let list_id = create_list(&state, "Reorder").await;
    for series_id in [a, b, c] {
        add(&state, list_id, series_id).await.unwrap();
    }

    let status = reorder_series(
        State(state),
        Path(list_id),
        Json(ReorderSeriesRequest {
            series_ids: vec![c, a, b],
        }),
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(position_of(&pool, list_id, c).await, Some(0));
    assert_eq!(position_of(&pool, list_id, a).await, Some(1));
    assert_eq!(position_of(&pool, list_id, b).await, Some(2));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reorder_series_rejects_unknown_ids(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "reorder-unknown").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let list_id = create_list(&state, "Reorder unknown").await;
    add(&state, list_id, a).await.unwrap();
    add(&state, list_id, b).await.unwrap();

    let err = reorder_series(
        State(state),
        Path(list_id),
        Json(ReorderSeriesRequest {
            series_ids: vec![a, Uuid::new_v4(), b],
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(position_of(&pool, list_id, a).await, Some(0));
    assert_eq!(position_of(&pool, list_id, b).await, Some(1));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reorder_series_rejects_partial_payload(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "reorder-partial").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let c = create_series(&pool, library_id, "C", &[]).await;
    let list_id = create_list(&state, "Reorder partial").await;
    for series_id in [a, b, c] {
        add(&state, list_id, series_id).await.unwrap();
    }

    let err = reorder_series(
        State(state),
        Path(list_id),
        Json(ReorderSeriesRequest {
            series_ids: vec![c, a],
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    // Nothing was moved: omitted series keep their positions, no collisions.
    assert_eq!(position_of(&pool, list_id, a).await, Some(0));
    assert_eq!(position_of(&pool, list_id, b).await, Some(1));
    assert_eq!(position_of(&pool, list_id, c).await, Some(2));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reorder_series_rejects_duplicate_ids(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "reorder-duplicates").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let list_id = create_list(&state, "Reorder duplicates").await;
    add(&state, list_id, a).await.unwrap();
    add(&state, list_id, b).await.unwrap();

    let err = reorder_series(
        State(state),
        Path(list_id),
        Json(ReorderSeriesRequest {
            series_ids: vec![a, a],
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(position_of(&pool, list_id, a).await, Some(0));
    assert_eq!(position_of(&pool, list_id, b).await, Some(1));
}

// ---------------------------------------------------------------------------
// moves
// ---------------------------------------------------------------------------

async fn move_to(
    state: &AppState,
    from_list_id: Uuid,
    series_id: Uuid,
    to_list_id: Uuid,
) -> Result<StatusCode, ApiError> {
    move_series(
        State(state.clone()),
        Path((from_list_id, series_id)),
        Json(MoveSeriesRequest { to_list_id }),
    )
    .await
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn move_series_compacts_both_lists(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "move").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let c = create_series(&pool, library_id, "C", &[]).await;
    let x = create_series(&pool, library_id, "X", &[]).await;
    let source = create_list(&state, "Source").await;
    let destination = create_list(&state, "Destination").await;
    for series_id in [a, b, c] {
        add(&state, source, series_id).await.unwrap();
    }
    add(&state, destination, x).await.unwrap();

    let status = move_to(&state, source, b, destination).await.unwrap();

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(positions_of(&pool, source).await, vec![0, 1]);
    assert_eq!(position_of(&pool, source, a).await, Some(0));
    assert_eq!(position_of(&pool, source, c).await, Some(1));
    assert_eq!(position_of(&pool, source, b).await, None);

    assert_eq!(positions_of(&pool, destination).await, vec![0, 1]);
    assert_eq!(position_of(&pool, destination, x).await, Some(0));
    assert_eq!(position_of(&pool, destination, b).await, Some(1));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn move_series_rejects_a_self_move(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "move-self").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let list_id = create_list(&state, "Only").await;
    add(&state, list_id, a).await.unwrap();

    let err = move_to(&state, list_id, a, list_id).await.unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(position_of(&pool, list_id, a).await, Some(0));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn move_series_rejects_an_unknown_destination_list(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "move-unknown-list").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let list_id = create_list(&state, "Source").await;
    add(&state, list_id, a).await.unwrap();

    let err = move_to(&state, list_id, a, Uuid::new_v4())
        .await
        .unwrap_err();

    assert_eq!(err.status, StatusCode::NOT_FOUND);
    assert_eq!(position_of(&pool, list_id, a).await, Some(0));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn move_series_rejects_a_series_absent_from_the_source_list(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "move-absent").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let source = create_list(&state, "Source").await;
    let destination = create_list(&state, "Destination").await;
    add(&state, source, a).await.unwrap();
    add(&state, destination, b).await.unwrap();

    let err = move_to(&state, source, b, destination).await.unwrap_err();

    assert_eq!(err.status, StatusCode::NOT_FOUND);
    assert_eq!(position_of(&pool, destination, b).await, Some(0));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn move_series_rejects_a_series_already_in_the_destination(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "move-duplicate").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let source = create_list(&state, "Source").await;
    let destination = create_list(&state, "Destination").await;
    add(&state, source, a).await.unwrap();
    add(&state, destination, a).await.unwrap();

    let err = move_to(&state, source, a, destination).await.unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    // The series is left untouched in both lists.
    assert_eq!(position_of(&pool, source, a).await, Some(0));
    assert_eq!(position_of(&pool, destination, a).await, Some(0));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn move_series_stays_consistent_when_racing_an_addition(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "move-race").await;
    let a = create_series(&pool, library_id, "A", &[]).await;
    let b = create_series(&pool, library_id, "B", &[]).await;
    let concurrent = create_series(&pool, library_id, "Concurrent", &[]).await;
    let source = create_list(&state, "Source").await;
    let destination = create_list(&state, "Destination").await;
    add(&state, source, a).await.unwrap();
    add(&state, source, b).await.unwrap();

    let (moved, added) = tokio::join!(
        move_to(&state, source, a, destination),
        add(&state, destination, concurrent),
    );
    moved.unwrap();
    added.unwrap();

    assert_eq!(positions_of(&pool, source).await, vec![0]);
    assert_eq!(positions_of(&pool, destination).await, vec![0, 1]);
    assert_eq!(item_count(&pool, source).await, 1);
    assert_eq!(item_count(&pool, destination).await, 2);
    assert_eq!(position_of(&pool, source, b).await, Some(0));
    assert!(position_of(&pool, destination, a).await.is_some());
    assert!(position_of(&pool, destination, concurrent).await.is_some());
}

// ---------------------------------------------------------------------------
// memberships + list
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_memberships_is_empty_without_items(pool: PgPool) {
    let state = test_state(pool);

    let Json(memberships) = get_memberships(State(state)).await.unwrap();

    assert!(memberships.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_memberships_returns_every_item(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "memberships").await;
    let series_id = create_series(&pool, library_id, "Shared", &[]).await;
    let first_list = create_list(&state, "First").await;
    let second_list = create_list(&state, "Second").await;
    add(&state, first_list, series_id).await.unwrap();
    add(&state, second_list, series_id).await.unwrap();

    let Json(memberships) = get_memberships(State(state)).await.unwrap();

    assert_eq!(memberships.len(), 2);
    assert!(
        memberships
            .iter()
            .all(|m| m.series_id == series_id
                && (m.list_id == first_list || m.list_id == second_list))
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_reading_lists_orders_by_name_and_counts_series(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "list-order").await;
    let first = create_series(&pool, library_id, "First", &[]).await;
    let second = create_series(&pool, library_id, "Second", &[]).await;
    create_list(&state, "Zeta").await;
    let alpha = create_list(&state, "Alpha").await;
    add(&state, alpha, first).await.unwrap();
    add(&state, alpha, second).await.unwrap();

    let Json(lists) = list_reading_lists(
        State(state),
        None,
        Query(ListReadingListsQuery { series_id: None }),
    )
    .await
    .unwrap();

    let names: Vec<&str> = lists.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "Zeta"]);
    assert_eq!(lists[0].series_count, 2);
    assert_eq!(lists[1].series_count, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_reading_lists_filters_by_series_membership(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "list-filter").await;
    let series_id = create_series(&pool, library_id, "Filtered", &[]).await;
    let containing = create_list(&state, "Containing").await;
    create_list(&state, "Other").await;
    add(&state, containing, series_id).await.unwrap();

    let Json(filtered) = list_reading_lists(
        State(state.clone()),
        None,
        Query(ListReadingListsQuery {
            series_id: Some(series_id),
        }),
    )
    .await
    .unwrap();
    let Json(all) = list_reading_lists(
        State(state),
        None,
        Query(ListReadingListsQuery { series_id: None }),
    )
    .await
    .unwrap();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, containing);
    assert_eq!(all.len(), 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_reading_lists_counts_read_books_per_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "list-progress").await;
    let series_id = create_series(&pool, library_id, "Progress", &[]).await;
    let first_book = create_book(&pool, library_id, series_id, "Vol 1").await;
    create_book(&pool, library_id, series_id, "Vol 2").await;
    let list_id = create_list(&state, "Progress list").await;
    add(&state, list_id, series_id).await.unwrap();

    let reader = create_user(&pool, "reader").await;
    let other = create_user(&pool, "other").await;
    mark_read(&pool, first_book, reader).await;

    let Json(reader_lists) = list_reading_lists(
        State(state.clone()),
        auth_user(reader),
        Query(ListReadingListsQuery { series_id: None }),
    )
    .await
    .unwrap();
    let Json(other_lists) = list_reading_lists(
        State(state.clone()),
        auth_user(other),
        Query(ListReadingListsQuery { series_id: None }),
    )
    .await
    .unwrap();
    let Json(guest_lists) = list_reading_lists(
        State(state),
        None,
        Query(ListReadingListsQuery { series_id: None }),
    )
    .await
    .unwrap();

    assert_eq!(reader_lists[0].book_count, 2);
    assert_eq!(reader_lists[0].books_read_count, 1);
    assert_eq!(other_lists[0].books_read_count, 0);
    assert_eq!(guest_lists[0].books_read_count, 0);
}

// ---------------------------------------------------------------------------
// get (detail)
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_list_returns_not_found_for_unknown_id(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(get_reading_list(State(state), None, Path(Uuid::new_v4())).await);

    assert_eq!(err.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_list_returns_items_in_position_order(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "detail-order").await;
    let first = create_series(&pool, library_id, "First", &[]).await;
    let second = create_series(&pool, library_id, "Second", &[]).await;
    let list_id = create_list(&state, "Detail").await;
    add(&state, list_id, first).await.unwrap();
    add(&state, list_id, second).await.unwrap();

    let Json(detail) = get_reading_list(State(state), None, Path(list_id))
        .await
        .unwrap();

    let ids: Vec<Uuid> = detail.items.iter().map(|i| i.id).collect();
    let positions: Vec<i32> = detail.items.iter().map(|i| i.position).collect();
    assert_eq!(ids, vec![first, second]);
    assert_eq!(positions, vec![0, 1]);
    assert_eq!(detail.id, list_id);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_list_flags_series_without_books(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "detail-empty").await;
    let with_book = create_series(&pool, library_id, "With book", &[]).await;
    create_book(&pool, library_id, with_book, "Vol 1").await;
    let empty = create_series(&pool, library_id, "Empty", &[]).await;
    let list_id = create_list(&state, "Mixed").await;
    add(&state, list_id, with_book).await.unwrap();
    add(&state, list_id, empty).await.unwrap();

    let Json(detail) = get_reading_list(State(state), None, Path(list_id))
        .await
        .unwrap();

    let flags: Vec<(Uuid, bool)> = detail.items.iter().map(|i| (i.id, i.is_empty)).collect();
    assert_eq!(flags, vec![(with_book, false), (empty, true)]);
    // Empty entries are flagged, never removed automatically.
    assert_eq!(item_count(&pool, list_id).await, 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_list_filters_restricted_genres(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "detail-genres").await;
    let visible = create_series(&pool, library_id, "Visible", &["Adventure"]).await;
    let restricted = create_series(&pool, library_id, "Restricted", &["Mature"]).await;
    let list_id = create_list(&state, "Mixed").await;
    add(&state, list_id, visible).await.unwrap();
    add(&state, list_id, restricted).await.unwrap();

    let user_id = create_user(&pool, "restricted-reader").await;
    sqlx::query("INSERT INTO user_genre_restrictions (user_id, genre) VALUES ($1, 'Mature')")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    let Json(detail) = get_reading_list(State(state), auth_user(user_id), Path(list_id))
        .await
        .unwrap();

    assert_eq!(detail.items.len(), 1);
    assert_eq!(detail.items[0].id, visible);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_list_moves_fully_read_series_to_end(pool: PgPool) {
    let state = test_state(pool.clone());
    let library_id = create_library(&pool, "detail-progress").await;
    let unfinished = create_series(&pool, library_id, "Unfinished", &[]).await;
    let finished = create_series(&pool, library_id, "Finished", &[]).await;
    create_book(&pool, library_id, unfinished, "Unfinished Vol 1").await;
    let finished_book = create_book(&pool, library_id, finished, "Finished Vol 1").await;
    let list_id = create_list(&state, "Reading order").await;
    // `finished` is added first (position 0) to prove the read-series reordering.
    add(&state, list_id, finished).await.unwrap();
    add(&state, list_id, unfinished).await.unwrap();

    let user_id = create_user(&pool, "finisher").await;
    mark_read(&pool, finished_book, user_id).await;

    let Json(detail) = get_reading_list(State(state), auth_user(user_id), Path(list_id))
        .await
        .unwrap();

    let ids: Vec<Uuid> = detail.items.iter().map(|i| i.id).collect();
    assert_eq!(ids, vec![unfinished, finished]);
    // Stored positions are untouched; only the response ordering changes.
    assert_eq!(position_of(&pool, list_id, finished).await, Some(0));
}
