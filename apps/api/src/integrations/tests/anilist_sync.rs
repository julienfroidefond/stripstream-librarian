//! Tests for the AniList progress sync helpers (preview / push stats and pull).

use axum::extract::State;
use uuid::Uuid;

use super::*;
use crate::state::test_state;

async fn seed_series(pool: &sqlx::PgPool) -> (Uuid, Uuid) {
    let lib: Uuid = sqlx::query_scalar(
        "INSERT INTO libraries (id, name, root_path, reading_status_provider) \
         VALUES (gen_random_uuid(), 'anilist_lib', '/libraries/anilist_lib', 'anilist') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let series: Uuid = sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, total_volumes) \
         VALUES (gen_random_uuid(), $1, 'Gon', 3) RETURNING id",
    )
    .bind(lib)
    .fetch_one(pool)
    .await
    .unwrap();
    (lib, series)
}

async fn seed_book(
    pool: &sqlx::PgPool,
    lib: Uuid,
    series: Uuid,
    volume: Option<i32>,
    volume_type: &str,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books (id, library_id, series_id, title, kind, format, volume, volume_type) \
         VALUES (gen_random_uuid(), $1, $2, $3, 'comic', 'cbz', $4, $5) RETURNING id",
    )
    .bind(lib)
    .bind(series)
    .bind(format!("Gon {volume:?} {volume_type}"))
    .bind(volume)
    .bind(volume_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_user(pool: &sqlx::PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (id, username) VALUES (gen_random_uuid(), 'anilist_user') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn set_status(pool: &sqlx::PgPool, book: Uuid, user: Uuid, status: &str) {
    sqlx::query(
        "INSERT INTO book_reading_progress (book_id, user_id, status) VALUES ($1, $2, $3) \
         ON CONFLICT (book_id, user_id) DO UPDATE SET status = EXCLUDED.status",
    )
    .bind(book)
    .bind(user)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

async fn status_of(pool: &sqlx::PgPool, book: Uuid, user: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
    )
    .bind(book)
    .bind(user)
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[test]
fn pulled_statuses_follow_progress_volumes() {
    assert_eq!(
        pulled_book_statuses("reading", 1, 3),
        vec!["read", "reading", "unread"]
    );
    assert_eq!(pulled_book_statuses("read", 0, 2), vec!["read", "read"]);
    assert_eq!(
        pulled_book_statuses("unread", 0, 2),
        vec!["unread", "unread"]
    );
    // Negative or oversized progress is clamped.
    assert_eq!(pulled_book_statuses("reading", -4, 1), vec!["reading"]);
    assert_eq!(pulled_book_statuses("reading", 10, 2), vec!["read", "read"]);
}

#[test]
fn anilist_progress_is_derived_from_stats() {
    let stats = |books_read, total| SeriesReadingStats {
        book_count: 3,
        books_read,
        total_volumes: total,
    };
    assert_eq!(stats(0, Some(3)).anilist_progress(), ("PLANNING", 0));
    assert_eq!(stats(2, Some(3)).anilist_progress(), ("CURRENT", 2));
    assert_eq!(stats(3, Some(3)).anilist_progress(), ("COMPLETED", 3));
    assert_eq!(stats(3, None).anilist_progress(), ("CURRENT", 3));
}

#[sqlx::test(migrations = "../../infra/migrations")]
/// Same scope as the status push: integrals count, specials do not.
async fn stats_count_regular_and_integral_volumes(pool: sqlx::PgPool) {
    let (lib, series) = seed_series(&pool).await;
    let user = seed_user(&pool).await;
    let t1 = seed_book(&pool, lib, series, Some(1), "regular").await;
    let hs = seed_book(&pool, lib, series, None, "hs").await;
    let integral = seed_book(&pool, lib, series, None, "integral").await;
    seed_book(&pool, lib, series, Some(2), "regular").await;
    set_status(&pool, t1, user, "read").await;
    set_status(&pool, hs, user, "read").await;
    set_status(&pool, integral, user, "read").await;

    let stats = series_reading_stats(&pool, series, user).await.unwrap();

    assert_eq!(
        stats,
        SeriesReadingStats {
            book_count: 3,
            books_read: 2,
            total_volumes: Some(3),
        }
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn preview_ignores_non_regular_volumes(pool: sqlx::PgPool) {
    let (lib, series) = seed_series(&pool).await;
    let user = seed_user(&pool).await;
    let t1 = seed_book(&pool, lib, series, Some(1), "regular").await;
    let hs = seed_book(&pool, lib, series, None, "hs").await;
    set_status(&pool, t1, user, "read").await;
    set_status(&pool, hs, user, "read").await;
    sqlx::query(
        "INSERT INTO anilist_series_links (library_id, series_id, anilist_id) VALUES ($1, $2, 42)",
    )
    .bind(lib)
    .bind(series)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('anilist', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::json!({ "access_token": "t", "local_user_id": user.to_string() }))
    .execute(&pool)
    .await
    .unwrap();

    let items = preview_sync(State(test_state(pool.clone())))
        .await
        .unwrap()
        .0;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].book_count, 1);
    assert_eq!(items[0].books_read, 1);
    assert_eq!(items[0].progress_volumes, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn pull_never_downgrades_local_progress(pool: sqlx::PgPool) {
    let (lib, series) = seed_series(&pool).await;
    let user = seed_user(&pool).await;
    let t1 = seed_book(&pool, lib, series, Some(1), "regular").await;
    let t2 = seed_book(&pool, lib, series, Some(2), "regular").await;
    let t3 = seed_book(&pool, lib, series, Some(3), "regular").await;
    set_status(&pool, t1, user, "read").await;
    set_status(&pool, t2, user, "read").await;
    set_status(&pool, t3, user, "reading").await;

    // AniList says PLANNING / progressVolumes = 0: nothing must move back.
    let updated = apply_pulled_progress(&pool, series, user, "unread", 0)
        .await
        .unwrap();
    assert_eq!(updated, Some(0));
    // AniList behind the local progress: still nothing moves back.
    let updated = apply_pulled_progress(&pool, series, user, "reading", 0)
        .await
        .unwrap();
    assert_eq!(updated, Some(0));

    assert_eq!(status_of(&pool, t1, user).await.as_deref(), Some("read"));
    assert_eq!(status_of(&pool, t2, user).await.as_deref(), Some("read"));
    assert_eq!(status_of(&pool, t3, user).await.as_deref(), Some("reading"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn pull_applies_advances_on_regular_volumes_only(pool: sqlx::PgPool) {
    let (lib, series) = seed_series(&pool).await;
    let user = seed_user(&pool).await;
    let t1 = seed_book(&pool, lib, series, Some(1), "regular").await;
    let t2 = seed_book(&pool, lib, series, Some(2), "regular").await;
    let t3 = seed_book(&pool, lib, series, Some(3), "regular").await;
    let hs = seed_book(&pool, lib, series, Some(1), "hs").await;
    set_status(&pool, t1, user, "reading").await;

    let updated = apply_pulled_progress(&pool, series, user, "reading", 1)
        .await
        .unwrap();

    // t1: reading -> read, t2: none -> reading, t3 untouched (unread is never written).
    assert_eq!(updated, Some(2));
    assert_eq!(status_of(&pool, t1, user).await.as_deref(), Some("read"));
    assert_eq!(status_of(&pool, t2, user).await.as_deref(), Some("reading"));
    assert_eq!(status_of(&pool, t3, user).await, None);
    assert_eq!(status_of(&pool, hs, user).await, None);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn pull_skips_series_without_regular_books(pool: sqlx::PgPool) {
    let (lib, series) = seed_series(&pool).await;
    let user = seed_user(&pool).await;
    seed_book(&pool, lib, series, None, "oneshot").await;

    let updated = apply_pulled_progress(&pool, series, user, "read", 1)
        .await
        .unwrap();
    assert_eq!(updated, None);
}
