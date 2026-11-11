//! Integration tests for series listing (`list_series`, `list_all_series`).
//!
//! Handlers are called directly with the shared `test_state` helper, backed by
//! an ephemeral Postgres provisioned by `#[sqlx::test]`. These lock the pagination
//! and `total` semantics after the count/data queries were merged into one.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::super::{
    get_series_by_id, list_all_series, list_series, ongoing_books, ongoing_series,
    ListAllSeriesQuery, ListSeriesQuery, OngoingQuery,
};
use crate::auth::AuthUser;
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

async fn create_book(pool: &PgPool, library_id: Uuid, series_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books \
             (id, library_id, kind, title, series_id, volume, volume_type, authors, page_count) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, 1, 'regular', '{}', 10) RETURNING id",
    )
    .bind(library_id)
    .bind(title)
    .bind(series_id)
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
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books \
             (id, library_id, kind, title, series_id, volume, volume_type, authors, page_count) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, $4, $5, '{}', 10) RETURNING id",
    )
    .bind(library_id)
    .bind(title)
    .bind(series_id)
    .bind(volume)
    .bind(volume_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_user(pool: &PgPool, username: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (id, username) VALUES (gen_random_uuid(), $1) RETURNING id",
    )
    .bind(username)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn set_progress(
    pool: &PgPool,
    book_id: Uuid,
    user_id: Uuid,
    status: &str,
    last_read_at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO book_reading_progress \
             (book_id, user_id, status, last_read_at, updated_at) \
         VALUES ($1, $2, $3, $4, $4)",
    )
    .bind(book_id)
    .bind(user_id)
    .bind(status)
    .bind(last_read_at)
    .execute(pool)
    .await
    .unwrap();
}

async fn set_read(pool: &PgPool, book_id: Uuid, user_id: Uuid, last_read_at: DateTime<Utc>) {
    set_progress(pool, book_id, user_id, "read", last_read_at).await;
}

async fn set_shelved(pool: &PgPool, series_id: Uuid, user_id: Uuid) {
    sqlx::query("INSERT INTO series_user_shelves (user_id, series_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(series_id)
        .execute(pool)
        .await
        .unwrap();
}

fn series_query(page: Option<i64>, limit: Option<i64>) -> Query<ListSeriesQuery> {
    Query(ListSeriesQuery {
        q: None,
        reading_status: None,
        series_status: None,
        has_missing: None,
        metadata_provider: None,
        has_books: None,
        shelved: None,
        page,
        limit,
        sort: None,
    })
}

fn all_query(page: Option<i64>, limit: Option<i64>) -> Query<ListAllSeriesQuery> {
    Query(ListAllSeriesQuery {
        q: None,
        library_id: None,
        reading_status: None,
        series_status: None,
        has_missing: None,
        metadata_provider: None,
        gap: None,
        author: None,
        page,
        limit,
        has_books: None,
        no_books: None,
        shelved: None,
        sort: None,
        genre: None,
        volume_type: None,
        rated_only: None,
    })
}

fn series_query_q(q: &str) -> Query<ListSeriesQuery> {
    Query(ListSeriesQuery {
        q: Some(q.to_string()),
        ..series_query(None, None).0
    })
}

fn all_query_q(q: &str) -> Query<ListAllSeriesQuery> {
    Query(ListAllSeriesQuery {
        q: Some(q.to_string()),
        ..all_query(None, None).0
    })
}

fn all_query_reading_status(status: &str) -> Query<ListAllSeriesQuery> {
    Query(ListAllSeriesQuery {
        reading_status: Some(status.to_string()),
        ..all_query(None, None).0
    })
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_total_counts_all_matching_rows(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    for name in ["A", "B", "C"] {
        create_series(&pool, library, name).await;
    }

    let Json(page) = list_series(State(state), None, Path(library), series_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_total_is_independent_of_page(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    for name in ["A", "B", "C", "D", "E"] {
        create_series(&pool, library, name).await;
    }

    let Json(first) = list_series(
        State(state.clone()),
        None,
        Path(library),
        series_query(Some(1), Some(2)),
    )
    .await
    .unwrap();
    let Json(second) = list_series(
        State(state),
        None,
        Path(library),
        series_query(Some(2), Some(2)),
    )
    .await
    .unwrap();

    assert_eq!(first.total, 5);
    assert_eq!(first.items.len(), 2);
    assert_eq!(second.total, 5);
    assert_eq!(second.items.len(), 2);
    assert_ne!(first.items[0].series_id, second.items[0].series_id);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_empty_page_still_reports_total(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_series(&pool, library, "A").await;

    let Json(page) = list_series(
        State(state),
        None,
        Path(library),
        series_query(Some(5), Some(10)),
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert!(page.items.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_counts_books_and_first_book(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let book = create_book(&pool, library, series, "Vol 1").await;

    let Json(page) = list_series(State(state), None, Path(library), series_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].book_count, 1);
    assert_eq!(page.items[0].first_book_id, Some(book));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_first_book_prefers_regular_then_lowest_volume(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;

    // Inserted out of order: the "first book" must be regular volume 1, not the
    // integral (which has a higher volume) nor regular volume 2.
    create_book_with(&pool, library, series, "Integral", Some(3), "integral").await;
    create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    let vol1 = create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;

    let Json(page) = list_series(State(state), None, Path(library), series_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].book_count, 3);
    assert_eq!(page.items[0].first_book_id, Some(vol1));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_first_book_prefers_regular_then_lowest_volume(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;

    create_book_with(&pool, library, series, "Integral", Some(3), "integral").await;
    create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    let vol1 = create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;

    let Json(page) = list_all_series(State(state), None, all_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].book_count, 3);
    assert_eq!(page.items[0].first_book_id, Some(vol1));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_exposes_latest_reading_activity(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book(&pool, library, series, "Vol 1").await;
    let vol2 = create_book(&pool, library, series, "Vol 2").await;
    let user = create_user(&pool, "reader").await;

    let older = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let newer = DateTime::parse_from_rfc3339("2026-02-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, older).await;
    set_read(&pool, vol2, user, newer).await;

    let Json(page) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        all_query(None, None),
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].books_read_count, 2);
    assert_eq!(page.items[0].last_read_at, Some(newer));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_last_read_at_is_user_scoped(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book(&pool, library, series, "Vol 1").await;
    let vol2 = create_book(&pool, library, series, "Vol 2").await;
    let alice = create_user(&pool, "alice").await;
    let bob = create_user(&pool, "bob").await;

    let alice_read = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let bob_read = DateTime::parse_from_rfc3339("2026-02-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, alice, alice_read).await;
    set_read(&pool, vol2, bob, bob_read).await;

    let Json(alice_page) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: alice })),
        all_query(None, None),
    )
    .await
    .unwrap();
    let Json(bob_page) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: bob })),
        all_query(None, None),
    )
    .await
    .unwrap();

    assert_eq!(alice_page.items[0].last_read_at, Some(alice_read));
    assert_eq!(alice_page.items[0].books_read_count, 1);
    assert_eq!(bob_page.items[0].last_read_at, Some(bob_read));
    assert_eq!(bob_page.items[0].books_read_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_last_read_at_is_null_without_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book(&pool, library, series, "Vol 1").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;

    let Json(page) = list_all_series(State(state), None, all_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.items[0].last_read_at, None);
    assert_eq!(page.items[0].books_read_count, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_last_read_at_ignores_unread_reset(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book(&pool, library, series, "Vol 1").await;
    let vol2 = create_book(&pool, library, series, "Vol 2").await;
    let user = create_user(&pool, "reader").await;

    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let reset_at = DateTime::parse_from_rfc3339("2026-02-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;
    set_progress(&pool, vol2, user, "unread", reset_at).await;

    let Json(page) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        all_query(None, None),
    )
    .await
    .unwrap();

    assert_eq!(page.items[0].last_read_at, Some(read_at));
    assert_eq!(page.items[0].books_read_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_exposes_last_read_at_for_current_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book(&pool, library, series, "Vol 1").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;

    let Json(page) = list_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        Path(library),
        series_query(None, None),
    )
    .await
    .unwrap();

    assert_eq!(page.items[0].last_read_at, Some(read_at));
    assert_eq!(page.items[0].books_read_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_series_by_id_exposes_last_read_at_for_current_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book(&pool, library, series, "Vol 1").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;

    let Json(item) = get_series_by_id(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        Path(series),
    )
    .await
    .unwrap();

    assert_eq!(item.last_read_at, Some(read_at));
    assert_eq!(item.books_read_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn ongoing_series_exposes_last_read_at_for_current_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;

    let Json(items) = ongoing_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        Query(OngoingQuery { limit: None }),
    )
    .await
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].last_read_at, Some(read_at));
    assert_eq!(items[0].books_read_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_series_by_id_uses_the_canonical_missing_count(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    sqlx::query("UPDATE series SET total_volumes = 10 WHERE id = $1")
        .bind(series)
        .execute(&pool)
        .await
        .unwrap();

    create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with(&pool, library, series, "INT 2", Some(2), "integral").await;
    create_book_with(&pool, library, series, "HS 3", Some(3), "hs").await;
    create_book_with(&pool, library, series, "One-shot 4", Some(4), "oneshot").await;
    create_book_with(&pool, library, series, "Special", Some(0), "regular").await;

    let Json(item) = get_series_by_id(State(state), None, Path(series))
        .await
        .unwrap();

    assert_eq!(
        item.missing_count,
        Some(8),
        "10 total minus the two counted tomes"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_total_counts_all_matching_rows(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    for name in ["A", "B", "C"] {
        create_series(&pool, library, name).await;
    }

    let Json(page) = list_all_series(State(state), None, all_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_total_is_independent_of_page(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    for name in ["A", "B", "C", "D", "E"] {
        create_series(&pool, library, name).await;
    }

    let Json(first) = list_all_series(State(state.clone()), None, all_query(Some(1), Some(2)))
        .await
        .unwrap();
    let Json(second) = list_all_series(State(state), None, all_query(Some(2), Some(2)))
        .await
        .unwrap();

    assert_eq!(first.total, 5);
    assert_eq!(first.items.len(), 2);
    assert_eq!(second.total, 5);
    assert_eq!(second.items.len(), 2);
    assert_ne!(first.items[0].series_id, second.items[0].series_id);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_empty_page_still_reports_total(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_series(&pool, library, "A").await;

    let Json(page) = list_all_series(State(state), None, all_query(Some(5), Some(10)))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert!(page.items.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_total_spans_libraries(pool: PgPool) {
    let state = test_state(pool.clone());
    let lib_a = create_library(&pool, "a").await;
    let lib_b = create_library(&pool, "b").await;
    create_series(&pool, lib_a, "A1").await;
    create_series(&pool, lib_a, "A2").await;
    create_series(&pool, lib_b, "B1").await;

    let Json(page) = list_all_series(State(state), None, all_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_q_is_accent_insensitive(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_series(&pool, library, "Astérix").await;
    create_series(&pool, library, "Tintin").await;

    let Json(page) = list_series(State(state), None, Path(library), series_query_q("asterix"))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].name, "Astérix");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_q_is_accent_insensitive(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_series(&pool, library, "Astérix").await;
    create_series(&pool, library, "Tintin").await;

    let Json(page) = list_all_series(State(state), None, all_query_q("asterix"))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].name, "Astérix");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_q_accepts_accented_query(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_series(&pool, library, "Asterix").await;
    create_series(&pool, library, "Tintin").await;

    let Json(page) = list_all_series(State(state), None, all_query_q("astérix"))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].name, "Asterix");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_and_list_all_series_expose_shelved_at(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "Shelved series").await;
    let user = create_user(&pool, "reader").await;

    sqlx::query("INSERT INTO series_user_shelves (user_id, series_id) VALUES ($1, $2)")
        .bind(user)
        .bind(series)
        .execute(&pool)
        .await
        .unwrap();

    let Json(page) = list_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        Path(library),
        series_query(None, None),
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].shelved_at.is_some());

    let Json(page) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        all_query(None, None),
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].shelved_at.is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_shelved_at_is_null_without_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_series(&pool, library, "Unshelved series").await;

    let Json(page) = list_series(State(state), None, Path(library), series_query(None, None))
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert!(page.items[0].shelved_at.is_none());
}

// ─── Hors-série excluded from reading progress ──────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_counts_ignore_unread_hs(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    let vol2 = create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    create_book_with(&pool, library, series, "HS 1", Some(1), "hs").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;
    set_read(&pool, vol2, user, read_at).await;

    // Every list surface exposes the same tome counters.
    let Json(page) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        all_query(None, None),
    )
    .await
    .unwrap();
    assert_eq!(page.items[0].book_count, 2, "HS must not count as a tome");
    assert_eq!(page.items[0].books_read_count, 2);

    let Json(page) = list_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        Path(library),
        series_query(None, None),
    )
    .await
    .unwrap();
    assert_eq!(page.items[0].book_count, 2);
    assert_eq!(page.items[0].books_read_count, 2);

    let Json(item) = get_series_by_id(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        Path(series),
    )
    .await
    .unwrap();
    assert_eq!(item.book_count, 2);
    assert_eq!(item.books_read_count, 2);

    // The series is fully read even though the HS is still unread.
    let Json(unread) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        all_query_reading_status("unread,reading"),
    )
    .await
    .unwrap();
    assert_eq!(unread.total, 0);

    let Json(read) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        all_query_reading_status("read"),
    )
    .await
    .unwrap();
    assert_eq!(read.total, 1);
    assert_eq!(read.items[0].series_id, series);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_counts_keep_unread_regular_as_reading(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    create_book_with(&pool, library, series, "HS 1", Some(1), "hs").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;

    let Json(unread) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        all_query_reading_status("unread,reading"),
    )
    .await
    .unwrap();
    assert_eq!(unread.total, 1, "the unread regular tome keeps it ongoing");
    assert_eq!(unread.items[0].book_count, 2);
    assert_eq!(unread.items[0].books_read_count, 1);

    let Json(read) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        all_query_reading_status("read"),
    )
    .await
    .unwrap();
    assert_eq!(read.total, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn hs_only_series_falls_back_to_total_counts(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "HS only").await;
    let hs1 = create_book_with(&pool, library, series, "HS 1", Some(1), "hs").await;
    let hs2 = create_book_with(&pool, library, series, "HS 2", Some(2), "hs").await;
    let user = create_user(&pool, "reader").await;

    let Json(page) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        all_query(None, None),
    )
    .await
    .unwrap();
    assert_eq!(page.items[0].book_count, 2, "fallback to every book");
    assert_eq!(page.items[0].books_read_count, 0);

    // The fallback keeps the series visible in the reader (`has_books=true`).
    let mut has_books_query = all_query(None, None).0;
    has_books_query.has_books = Some("true".to_string());
    let Json(page) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        Query(has_books_query),
    )
    .await
    .unwrap();
    assert_eq!(page.total, 1);

    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, hs1, user, read_at).await;
    set_read(&pool, hs2, user, read_at).await;

    let Json(read) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        all_query_reading_status("read"),
    )
    .await
    .unwrap();
    assert_eq!(read.total, 1);
    assert_eq!(read.items[0].books_read_count, 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn ongoing_series_ignores_unread_hs(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    let vol2 = create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    create_book_with(&pool, library, series, "HS 1", Some(1), "hs").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;
    set_read(&pool, vol2, user, read_at).await;

    let Json(items) = ongoing_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: user })),
        Query(OngoingQuery { limit: None }),
    )
    .await
    .unwrap();
    assert!(
        items.is_empty(),
        "a fully read series must not stay ongoing because of an unread HS"
    );

    let Json(books) = ongoing_books(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        Query(OngoingQuery { limit: None }),
    )
    .await
    .unwrap();
    assert!(books.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn ongoing_books_skips_hs_for_next_tome(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let vol1 = create_book_with(&pool, library, series, "Vol 1", Some(1), "regular").await;
    let vol2 = create_book_with(&pool, library, series, "Vol 2", Some(2), "regular").await;
    create_book_with(&pool, library, series, "HS 1", Some(1), "hs").await;
    let user = create_user(&pool, "reader").await;
    let read_at = DateTime::parse_from_rfc3339("2026-01-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    set_read(&pool, vol1, user, read_at).await;

    let Json(books) = ongoing_books(
        State(state),
        Some(Extension(AuthUser { user_id: user })),
        Query(OngoingQuery { limit: None }),
    )
    .await
    .unwrap();

    assert_eq!(books.len(), 1);
    assert_eq!(books[0].id, vol2, "the next tome must not be the HS");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_all_series_filters_shelved_for_current_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let shelved = create_series(&pool, library, "Shelved").await;
    let active = create_series(&pool, library, "Active").await;
    create_book(&pool, library, shelved, "Vol 1").await;
    create_book(&pool, library, active, "Vol 1").await;
    let reader = create_user(&pool, "reader").await;
    let other = create_user(&pool, "other").await;
    set_shelved(&pool, shelved, reader).await;

    let shelved_query = Query(ListAllSeriesQuery {
        shelved: Some("true".to_string()),
        ..all_query(None, None).0
    });
    let Json(page) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: reader })),
        shelved_query,
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].series_id, shelved);
    assert!(page.items[0].shelved_at.is_some());

    let active_query = Query(ListAllSeriesQuery {
        shelved: Some("false".to_string()),
        ..all_query(None, None).0
    });
    let Json(page) = list_all_series(
        State(state.clone()),
        Some(Extension(AuthUser { user_id: reader })),
        active_query,
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].series_id, active);
    assert!(page.items[0].shelved_at.is_none());

    // A shelf belongs to its reader: another user must not see it.
    let Json(page) = list_all_series(
        State(state),
        Some(Extension(AuthUser { user_id: other })),
        Query(ListAllSeriesQuery {
            shelved: Some("true".to_string()),
            ..all_query(None, None).0
        }),
    )
    .await
    .unwrap();

    assert_eq!(page.total, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_series_filters_shelved_for_current_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let shelved = create_series(&pool, library, "Shelved").await;
    let active = create_series(&pool, library, "Active").await;
    create_book(&pool, library, shelved, "Vol 1").await;
    create_book(&pool, library, active, "Vol 1").await;
    let reader = create_user(&pool, "reader").await;
    set_shelved(&pool, shelved, reader).await;

    let Json(page) = list_series(
        State(state),
        Some(Extension(AuthUser { user_id: reader })),
        Path(library),
        Query(ListSeriesQuery {
            shelved: Some("true".to_string()),
            ..series_query(None, None).0
        }),
    )
    .await
    .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].series_id, shelved);
    assert!(page.items[0].shelved_at.is_some());
}
