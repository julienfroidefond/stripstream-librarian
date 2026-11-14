//! `POST /series/mark-read`: marking a series unread deletes progress rows, so
//! it must also invalidate the AniList link of the series for the
//! differential push to pick the change up.

use axum::extract::{Extension, State};
use axum::Json;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::{mark_series_read, MarkSeriesReadRequest};
use crate::auth::AuthUser;
use crate::state::test_state;

async fn seed(pool: &PgPool) -> (Uuid, Uuid) {
    let lib: Uuid = sqlx::query_scalar(
        "INSERT INTO libraries (id, name, root_path) VALUES (gen_random_uuid(), 'L', '/libraries/l') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let series: Uuid =
        sqlx::query_scalar("INSERT INTO series (library_id, name) VALUES ($1, 'S') RETURNING id")
            .bind(lib)
            .fetch_one(pool)
            .await
            .unwrap();
    let book: Uuid = sqlx::query_scalar(
        "INSERT INTO books (id, library_id, series_id, kind, title, format) VALUES (gen_random_uuid(), $1, $2, 'bd', 'T1', 'cbz') RETURNING id",
    )
    .bind(lib)
    .bind(series)
    .fetch_one(pool)
    .await
    .unwrap();
    let user: Uuid = sqlx::query_scalar("INSERT INTO users (username) VALUES ('u') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO book_reading_progress (book_id, user_id, status, last_read_at, updated_at) \
         VALUES ($1, $2, 'read', NOW(), NOW() - INTERVAL '1 day')",
    )
    .bind(book)
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO anilist_series_links (library_id, series_id, anilist_id, synced_at) \
         VALUES ($1, $2, 42, NOW())",
    )
    .bind(lib)
    .bind(series)
    .execute(pool)
    .await
    .unwrap();
    (series, user)
}

async fn synced_at(pool: &PgPool, series: Uuid) -> Option<DateTime<Utc>> {
    sqlx::query_scalar("SELECT synced_at FROM anilist_series_links WHERE series_id = $1")
        .bind(series)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn mark(pool: &PgPool, series: Uuid, user: Uuid, status: &str) -> i64 {
    let Json(res) = mark_series_read(
        State(test_state(pool.clone())),
        Some(Extension(AuthUser { user_id: user })),
        Json(MarkSeriesReadRequest {
            series: series.to_string(),
            status: status.to_string(),
        }),
    )
    .await
    .unwrap();
    res.updated
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn mark_unread_invalidates_anilist_sync(pool: PgPool) {
    let (series, user) = seed(&pool).await;
    assert!(synced_at(&pool, series).await.is_some());

    assert_eq!(mark(&pool, series, user, "unread").await, 1);

    assert!(
        synced_at(&pool, series).await.is_none(),
        "the next differential push must include the series"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn mark_unread_without_progress_keeps_sync_marker(pool: PgPool) {
    let (series, user) = seed(&pool).await;
    sqlx::query("DELETE FROM book_reading_progress")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(mark(&pool, series, user, "unread").await, 0);
    assert!(synced_at(&pool, series).await.is_some());
}
