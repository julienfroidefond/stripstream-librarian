//! Integration tests for the `stats` module.
//!
//! Handlers are called directly with a hand-built [`AppState`] backed by an
//! ephemeral Postgres provisioned by `#[sqlx::test]`. This exercises the real
//! aggregation SQL (overview counters, per-period time series, per-user
//! scoping, reading overview JSON aggregation) without starting an HTTP
//! server.

use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};

use axum::{
    extract::{Extension, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};
use uuid::Uuid;

use super::*;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::{
    AppState, DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn test_state(pool: PgPool) -> AppState {
    AppState {
        pool,
        bootstrap_token: Arc::from("test-token"),
        page_cache: Arc::new(Mutex::new(crate::state::PageCache::new(1))),
        disk_cache_stats: Arc::new(Mutex::new(None::<DiskCacheStatsSnapshot>)),
        page_render_locks: Arc::new(PageRenderLocks::new(1)),
        page_render_limit: Arc::new(Semaphore::new(1)),
        metrics: Arc::new(Metrics {
            requests_total: AtomicU64::new(0),
            page_cache_hits: AtomicU64::new(0),
            page_cache_misses: AtomicU64::new(0),
        }),
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit::new())),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
    }
}

/// `unwrap_err` requires `T: Debug`, which the response DTOs do not implement.
#[allow(dead_code)]
fn expect_err<T>(result: Result<T, ApiError>) -> ApiError {
    match result {
        Ok(_) => panic!("expected an error result"),
        Err(err) => err,
    }
}

fn auth_user(user_id: Uuid) -> Option<Extension<AuthUser>> {
    Some(Extension(AuthUser { user_id }))
}

fn stats_query(period: Option<&str>) -> Query<StatsQuery> {
    Query(StatsQuery {
        period: period.map(str::to_string),
    })
}

// ---------------------------------------------------------------------------
// Seed helpers
// ---------------------------------------------------------------------------

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
        "INSERT INTO series (id, library_id, name, genres) \
         VALUES (gen_random_uuid(), $1, $2, '{}') RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_user(pool: &PgPool, username: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
async fn create_book(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Option<Uuid>,
    title: &str,
    kind: &str,
    authors: Vec<String>,
    author: Option<&str>,
    page_count: i32,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO books \
             (id, library_id, kind, title, series_id, volume, volume_type, authors, author, page_count) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, NULL, 'regular', $5, $6, $7) RETURNING id",
    )
    .bind(library_id)
    .bind(kind)
    .bind(title)
    .bind(series_id)
    .bind(authors)
    .bind(author)
    .bind(page_count)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_simple_book(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Option<Uuid>,
    title: &str,
    page_count: i32,
) -> Uuid {
    create_book(
        pool,
        library_id,
        series_id,
        title,
        "comic",
        Vec::new(),
        None,
        page_count,
    )
    .await
}

async fn set_book_language(pool: &PgPool, book_id: Uuid, language: Option<&str>) {
    sqlx::query("UPDATE books SET language = $1 WHERE id = $2")
        .bind(language)
        .bind(book_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn set_book_summary_isbn(
    pool: &PgPool,
    book_id: Uuid,
    summary: Option<&str>,
    isbn: Option<&str>,
) {
    sqlx::query("UPDATE books SET summary = $1, isbn = $2 WHERE id = $3")
        .bind(summary)
        .bind(isbn)
        .bind(book_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn insert_file(
    pool: &PgPool,
    book_id: Uuid,
    format: &str,
    size_bytes: i64,
    updated_at: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO book_files \
             (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status, updated_at) \
         VALUES ($1, $2, $3, $4, $5, NOW(), $6, 'ok', $7)",
    )
    .bind(id)
    .bind(book_id)
    .bind(format)
    .bind(format!("/libraries/{}.{}", id, format))
    .bind(size_bytes)
    .bind(id.to_string())
    .bind(updated_at)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn set_progress(
    pool: &PgPool,
    book_id: Uuid,
    user_id: Uuid,
    status: &str,
    current_page: Option<i32>,
    last_read_at: Option<DateTime<Utc>>,
) {
    sqlx::query(
        "INSERT INTO book_reading_progress \
             (book_id, user_id, status, current_page, last_read_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, NOW()) \
         ON CONFLICT (book_id, user_id) DO UPDATE \
             SET status = EXCLUDED.status, \
                 current_page = EXCLUDED.current_page, \
                 last_read_at = EXCLUDED.last_read_at, \
                 updated_at = NOW()",
    )
    .bind(book_id)
    .bind(user_id)
    .bind(status)
    .bind(current_page)
    .bind(last_read_at)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_link(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    provider: &str,
    status: &str,
) {
    sqlx::query(
        "INSERT INTO external_metadata_links \
             (library_id, series_id, provider, external_id, status) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(provider)
    .bind(format!("ext-{}", provider))
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_job(
    pool: &PgPool,
    library_id: Option<Uuid>,
    job_type: &str,
    status: &str,
    finished_at: Option<DateTime<Utc>>,
) {
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at, finished_at) \
         VALUES (gen_random_uuid(), $1, $2, $3, NOW(), $4)",
    )
    .bind(library_id)
    .bind(job_type)
    .bind(status)
    .bind(finished_at)
    .execute(pool)
    .await
    .unwrap();
}

async fn create_torrent(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    status: &str,
    expected_volumes: Vec<i32>,
    created_at: DateTime<Utc>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO torrent_downloads \
             (id, library_id, series_name, expected_volumes, status, created_at) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(library_id)
    .bind(series_name)
    .bind(expected_volumes)
    .bind(status)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_available(pool: &PgPool, library_id: Uuid, series_id: Uuid, missing_count: i32) {
    sqlx::query(
        "INSERT INTO available_downloads \
             (id, library_id, series_id, missing_count, available_releases, updated_at) \
         VALUES (gen_random_uuid(), $1, $2, $3, '[]'::jsonb, NOW())",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(missing_count)
    .execute(pool)
    .await
    .unwrap();
}

// ---------------------------------------------------------------------------
// Projection helpers — the response DTOs do not implement PartialEq/Debug.
// ---------------------------------------------------------------------------

fn formats(items: &[FormatCount]) -> Vec<(String, i64)> {
    items.iter().map(|f| (f.format.clone(), f.count)).collect()
}

fn languages(items: &[LanguageCount]) -> Vec<(Option<String>, i64)> {
    items
        .iter()
        .map(|l| (l.language.clone(), l.count))
        .collect()
}

fn providers(items: &[ProviderCount]) -> Vec<(String, i64)> {
    items
        .iter()
        .map(|p| (p.provider.clone(), p.count))
        .collect()
}

fn job_totals(items: &[JobTimePoint]) -> (i64, i64, i64, i64, i64, i64, i64) {
    items.iter().fold((0, 0, 0, 0, 0, 0, 0), |acc, p| {
        (
            acc.0 + p.scan,
            acc.1 + p.rebuild,
            acc.2 + p.thumbnail,
            acc.3 + p.metadata,
            acc.4 + p.downloads,
            acc.5 + p.reading,
            acc.6 + p.conversion,
        )
    })
}

fn assert_overview_eq(a: &StatsOverview, b: &StatsOverview) {
    assert_eq!(a.total_books, b.total_books);
    assert_eq!(a.total_series, b.total_series);
    assert_eq!(a.total_libraries, b.total_libraries);
    assert_eq!(a.total_pages, b.total_pages);
    assert_eq!(a.total_size_bytes, b.total_size_bytes);
    assert_eq!(a.total_authors, b.total_authors);
}

// ---------------------------------------------------------------------------
// get_stats
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_empty_db_returns_zeroed_counts(pool: PgPool) {
    let state = test_state(pool.clone());

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(resp.overview.total_books, 0);
    assert_eq!(resp.overview.total_series, 0);
    assert_eq!(resp.overview.total_libraries, 0);
    assert_eq!(resp.overview.total_pages, 0);
    assert_eq!(resp.overview.total_size_bytes, 0);
    assert_eq!(resp.overview.total_authors, 0);

    assert_eq!(resp.reading_status.unread, 0);
    assert_eq!(resp.reading_status.reading, 0);
    assert_eq!(resp.reading_status.read, 0);

    assert!(resp.currently_reading.is_empty());
    assert!(resp.recently_read.is_empty());
    assert!(resp.by_format.is_empty());
    assert!(resp.by_language.is_empty());
    assert!(resp.by_library.is_empty());
    assert!(resp.top_series.is_empty());

    assert_eq!(resp.metadata.total_series, 0);
    assert_eq!(resp.metadata.series_linked, 0);
    assert_eq!(resp.metadata.series_unlinked, 0);
    assert_eq!(resp.metadata.books_with_summary, 0);
    assert_eq!(resp.metadata.books_with_isbn, 0);
    assert!(resp.metadata.by_provider.is_empty());

    assert_eq!(resp.downloads.active_downloads, 0);
    assert_eq!(resp.downloads.total_downloads, 0);
    assert!(resp.downloads.recent_downloads.is_empty());

    // Time series are produced by generate_series even on an empty DB.
    assert!(!resp.additions_over_time.is_empty());
    assert!(resp.additions_over_time.iter().all(|p| p.books_added == 0));
    assert!(!resp.reading_over_time.is_empty());
    assert!(resp
        .reading_over_time
        .iter()
        .all(|p| p.books_read == 0 && p.pages_read == 0));
    assert!(!resp.jobs_over_time.is_empty());
    assert_eq!(job_totals(&resp.jobs_over_time), (0, 0, 0, 0, 0, 0, 0));

    // No users → no per-user series.
    assert!(resp.users_reading_over_time.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_counts_books_series_libraries_pages_and_authors(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let s1 = create_series(&pool, library, "S1").await;
    let _s2_empty = create_series(&pool, library, "S2-empty").await;

    let _b1 = create_book(
        &pool,
        library,
        Some(s1),
        "B1",
        "comic",
        vec!["A".to_string(), "B".to_string()],
        None,
        100,
    )
    .await;
    let _b2 = create_book(
        &pool,
        library,
        Some(s1),
        "B2",
        "comic",
        vec!["A".to_string()],
        None,
        50,
    )
    .await;
    let _b3 = create_book(
        &pool,
        library,
        None,
        "B3",
        "comic",
        Vec::new(),
        Some("C"),
        10,
    )
    .await;

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(resp.overview.total_books, 3);
    // Empty series are not counted: only s1 is attached to books.
    assert_eq!(resp.overview.total_series, 1);
    assert_eq!(resp.overview.total_libraries, 1);
    assert_eq!(resp.overview.total_pages, 160);
    // "A" appears twice in arrays but counts once; scalar author "C" is merged.
    assert_eq!(resp.overview.total_authors, 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_reading_status_and_reading_sections(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let user = create_user(&pool, "alice").await;

    let b1 = create_simple_book(&pool, library, Some(series), "B1", 120).await;
    let b2 = create_simple_book(&pool, library, Some(series), "B2", 42).await;
    let _b3 = create_simple_book(&pool, library, Some(series), "B3", 10).await;

    let now = Utc::now();
    set_progress(&pool, b1, user, "read", None, Some(now)).await;
    set_progress(&pool, b2, user, "reading", Some(5), None).await;

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(resp.reading_status.unread, 1);
    assert_eq!(resp.reading_status.reading, 1);
    assert_eq!(resp.reading_status.read, 1);

    assert_eq!(resp.currently_reading.len(), 1);
    let current = &resp.currently_reading[0];
    assert_eq!(current.book_id, b2.to_string());
    assert_eq!(current.title, "B2");
    assert_eq!(current.series.as_deref(), Some("S1"));
    assert_eq!(current.current_page, 5);
    assert_eq!(current.page_count, 42);
    assert_eq!(current.username.as_deref(), Some("alice"));

    assert_eq!(resp.recently_read.len(), 1);
    let recent = &resp.recently_read[0];
    assert_eq!(recent.book_id, b1.to_string());
    assert_eq!(recent.title, "B1");
    assert_eq!(recent.username.as_deref(), Some("alice"));
    assert_eq!(recent.last_read_at.len(), 10);

    // The read book lands in the current week bucket.
    let books_read: i64 = resp.reading_over_time.iter().map(|p| p.books_read).sum();
    let pages_read: i64 = resp.reading_over_time.iter().map(|p| p.pages_read).sum();
    assert_eq!(books_read, 1);
    assert_eq!(pages_read, 120);

    // One user → one row per generated week bucket.
    assert!(!resp.users_reading_over_time.is_empty());
    assert!(resp
        .users_reading_over_time
        .iter()
        .all(|p| p.username == "alice"));
    let user_books: i64 = resp
        .users_reading_over_time
        .iter()
        .map(|p| p.books_read)
        .sum();
    assert_eq!(user_books, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_scopes_reading_to_authenticated_user(pool: PgPool) {
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let alice = create_user(&pool, "alice").await;
    let bob = create_user(&pool, "bob").await;

    let b1 = create_simple_book(&pool, library, Some(series), "B1", 30).await;
    let b2 = create_simple_book(&pool, library, Some(series), "B2", 40).await;

    set_progress(&pool, b1, alice, "reading", Some(3), None).await;
    set_progress(&pool, b2, bob, "reading", Some(4), None).await;

    let state = test_state(pool.clone());
    let Json(scoped) = get_stats(State(state.clone()), stats_query(None), auth_user(alice))
        .await
        .unwrap();

    assert_eq!(scoped.reading_status.reading, 1);
    assert_eq!(scoped.reading_status.unread, 1);
    assert_eq!(scoped.currently_reading.len(), 1);
    assert_eq!(scoped.currently_reading[0].book_id, b1.to_string());
    assert_eq!(
        scoped.currently_reading[0].username.as_deref(),
        Some("alice")
    );

    let Json(unscoped) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();
    assert_eq!(unscoped.reading_status.reading, 2);
    assert_eq!(unscoped.currently_reading.len(), 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_users_reading_over_time_is_not_scoped_to_user(pool: PgPool) {
    let library = create_library(&pool, "main").await;
    let alice = create_user(&pool, "alice").await;
    let bob = create_user(&pool, "bob").await;

    let b1 = create_simple_book(&pool, library, None, "B1", 10).await;
    let b2 = create_simple_book(&pool, library, None, "B2", 10).await;
    let now = Utc::now();
    set_progress(&pool, b1, alice, "read", None, Some(now)).await;
    set_progress(&pool, b2, bob, "read", None, Some(now)).await;

    let state = test_state(pool.clone());
    let Json(scoped) = get_stats(State(state), stats_query(None), auth_user(alice))
        .await
        .unwrap();

    assert_eq!(scoped.reading_status.read, 1);
    let own_read: i64 = scoped.reading_over_time.iter().map(|p| p.books_read).sum();
    assert_eq!(own_read, 1);

    let mut usernames: Vec<String> = scoped
        .users_reading_over_time
        .iter()
        .map(|p| p.username.clone())
        .collect();
    usernames.sort();
    usernames.dedup();
    assert_eq!(usernames, vec!["alice".to_string()]);
    let own_users_read: i64 = scoped
        .users_reading_over_time
        .iter()
        .map(|p| p.books_read)
        .sum();
    assert_eq!(own_users_read, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_by_format_uses_latest_file_then_kind_fallback(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;

    let b1 = create_simple_book(&pool, library, None, "B1", 10).await;
    let _b2 = create_simple_book(&pool, library, None, "B2", 10).await;
    let _b3 = create_simple_book(&pool, library, None, "B3", 10).await;

    let now = Utc::now();
    insert_file(&pool, b1, "cbz", 100, now - Duration::hours(2)).await;
    insert_file(&pool, b1, "cbr", 200, now).await;

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(
        formats(&resp.by_format),
        vec![("unknown".to_string(), 2), ("cbr".to_string(), 1)]
    );
    // Only the most recent file per book counts toward the total size.
    assert_eq!(resp.overview.total_size_bytes, 200);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_by_language_includes_null_bucket(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;

    for (title, language) in [
        ("F1", Some("fr")),
        ("F2", Some("fr")),
        ("F3", Some("fr")),
        ("E1", Some("en")),
        ("E2", Some("en")),
        ("N1", None),
    ] {
        let id = create_simple_book(&pool, library, None, title, 10).await;
        set_book_language(&pool, id, language).await;
    }

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(
        languages(&resp.by_language),
        vec![
            (Some("fr".to_string()), 3),
            (Some("en".to_string()), 2),
            (None, 1),
        ]
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_by_library_counts_sizes_and_status(pool: PgPool) {
    let state = test_state(pool.clone());
    let lib_a = create_library(&pool, "A").await;
    let lib_b = create_library(&pool, "B").await;
    let user = create_user(&pool, "alice").await;

    let a1 = create_simple_book(&pool, lib_a, None, "A1", 10).await;
    let a2 = create_simple_book(&pool, lib_a, None, "A2", 10).await;
    let _b1 = create_simple_book(&pool, lib_b, None, "B1", 10).await;

    let now = Utc::now();
    insert_file(&pool, a1, "cbz", 10, now).await;
    insert_file(&pool, a2, "cbz", 20, now).await;
    set_progress(&pool, a1, user, "read", None, Some(now)).await;

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(resp.by_library.len(), 2);
    let a = &resp.by_library[0];
    assert_eq!(a.library_name, "A");
    assert_eq!(a.book_count, 2);
    assert_eq!(a.size_bytes, 30);
    assert_eq!(a.read_count, 1);
    assert_eq!(a.reading_count, 0);
    assert_eq!(a.unread_count, 1);

    let b = &resp.by_library[1];
    assert_eq!(b.library_name, "B");
    assert_eq!(b.book_count, 1);
    assert_eq!(b.size_bytes, 0);
    assert_eq!(b.unread_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_top_series_orders_by_book_count(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let popular = create_series(&pool, library, "Popular").await;
    let lonely = create_series(&pool, library, "Lonely").await;
    let user = create_user(&pool, "alice").await;

    let p1 = create_simple_book(&pool, library, Some(popular), "P1", 10).await;
    let _p2 = create_simple_book(&pool, library, Some(popular), "P2", 20).await;
    let _p3 = create_simple_book(&pool, library, Some(popular), "P3", 30).await;
    let _l1 = create_simple_book(&pool, library, Some(lonely), "L1", 5).await;
    set_progress(&pool, p1, user, "read", None, Some(Utc::now())).await;

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(resp.top_series.len(), 2);
    assert_eq!(resp.top_series[0].series, "Popular");
    assert_eq!(resp.top_series[0].book_count, 3);
    assert_eq!(resp.top_series[0].read_count, 1);
    assert_eq!(resp.top_series[0].total_pages, 60);
    assert_eq!(resp.top_series[1].series, "Lonely");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_metadata_tracks_links_summaries_and_providers(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let s1 = create_series(&pool, library, "S1").await;
    let s2 = create_series(&pool, library, "S2").await;

    let b1 = create_simple_book(&pool, library, Some(s1), "B1", 10).await;
    let _b2 = create_simple_book(&pool, library, Some(s2), "B2", 10).await;
    let _b3 = create_simple_book(&pool, library, None, "B3", 10).await;
    set_book_summary_isbn(&pool, b1, Some("summary"), Some("isbn-1")).await;

    create_link(&pool, library, s1, "comicvine", "approved").await;
    // Same series, second provider: distinct series per provider is still 1.
    create_link(&pool, library, s1, "bedetheque", "approved").await;
    // Pending links are ignored by linked counts and provider counts.
    create_link(&pool, library, s2, "comicvine", "pending").await;

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(resp.metadata.total_series, 2);
    assert_eq!(resp.metadata.series_linked, 1);
    assert_eq!(resp.metadata.series_unlinked, 1);
    assert_eq!(resp.metadata.books_with_summary, 1);
    assert_eq!(resp.metadata.books_with_isbn, 1);

    let mut by_provider = providers(&resp.metadata.by_provider);
    by_provider.sort();
    assert_eq!(
        by_provider,
        vec![("bedetheque".to_string(), 1), ("comicvine".to_string(), 1)]
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_period_shapes_and_rejects_invalid(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let _b1 = create_simple_book(&pool, library, None, "B1", 10).await;

    let Json(day) = get_stats(State(state.clone()), stats_query(Some("day")), None)
        .await
        .unwrap();
    assert_eq!(day.additions_over_time.len(), 7);
    assert!(day.additions_over_time.iter().all(|p| p.month.len() == 10));
    assert_eq!(day.additions_over_time.last().unwrap().books_added, 1);

    let Json(week) = get_stats(State(state.clone()), stats_query(Some("week")), None)
        .await
        .unwrap();
    assert!((8..=10).contains(&week.additions_over_time.len()));
    assert!(week.additions_over_time.iter().all(|p| p.month.len() == 10));
    assert_eq!(week.additions_over_time.last().unwrap().books_added, 1);

    let Json(month) = get_stats(State(state.clone()), stats_query(Some("month")), None)
        .await
        .unwrap();
    assert_eq!(month.additions_over_time.len(), 12);
    assert!(month.additions_over_time.iter().all(|p| p.month.len() == 7));
    assert_eq!(month.additions_over_time.last().unwrap().books_added, 1);

    let err = get_stats(State(state), stats_query(Some("yearly")), None)
        .await
        .err()
        .unwrap();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_jobs_over_time_categorizes_job_types(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let now = Utc::now();

    for job_type in [
        "scan",
        "rebuild",
        "full_rebuild",
        "rescan",
        "thumbnail_rebuild",
        "thumbnail_regenerate",
        "metadata_batch",
        "metadata_batch_rematch",
        "metadata_refresh",
        "metadata_refresh_all",
        "download_detection",
        "reading_status_match",
        "reading_status_push",
        "cbr_to_cbz",
    ] {
        create_job(&pool, Some(library), job_type, "success", Some(now)).await;
    }
    create_job(&pool, Some(library), "scan", "failed", Some(now)).await;
    create_job(&pool, Some(library), "prowlarr_rss", "success", Some(now)).await;

    create_job(&pool, Some(library), "scan", "running", Some(now)).await;
    create_job(&pool, Some(library), "scan", "success", None).await;

    let Json(resp) = get_stats(State(state), stats_query(Some("month")), None)
        .await
        .unwrap();

    assert_eq!(job_totals(&resp.jobs_over_time), (2, 3, 2, 4, 2, 2, 1));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_downloads_counts_and_recent_items(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let s1 = create_series(&pool, library, "S1").await;
    let s2 = create_series(&pool, library, "S2").await;

    let now = Utc::now();
    for (idx, status) in [
        "downloading",
        "importing",
        "imported",
        "error",
        "completed",
        "completed",
    ]
    .iter()
    .enumerate()
    {
        create_torrent(
            &pool,
            library,
            &format!("T{}", idx),
            status,
            vec![1, 2],
            now - Duration::minutes(idx as i64),
        )
        .await;
    }

    create_available(&pool, library, s1, 3).await;
    create_available(&pool, library, s2, 5).await;

    let Json(resp) = get_stats(State(state), stats_query(None), None)
        .await
        .unwrap();

    assert_eq!(resp.downloads.active_downloads, 2);
    assert_eq!(resp.downloads.imported_downloads, 1);
    assert_eq!(resp.downloads.error_downloads, 1);
    assert_eq!(resp.downloads.total_downloads, 6);
    assert_eq!(resp.downloads.available_series, 2);
    assert_eq!(resp.downloads.total_missing_volumes, 8);

    assert_eq!(resp.downloads.recent_downloads.len(), 5);
    assert_eq!(resp.downloads.recent_downloads[0].series_name, "T0");
    assert_eq!(resp.downloads.recent_downloads[0].status, "downloading");
    assert_eq!(
        resp.downloads.recent_downloads[0].expected_volumes,
        vec![1, 2]
    );
    assert_eq!(resp.downloads.recent_downloads[0].created_at.len(), 10);
}

// ---------------------------------------------------------------------------
// get_stats_overview
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_overview_empty_db_returns_zeroed_subset(pool: PgPool) {
    let state = test_state(pool.clone());

    let Json(resp) = get_stats_overview(State(state), None).await.unwrap();

    assert_eq!(resp.overview.total_books, 0);
    assert_eq!(resp.overview.total_size_bytes, 0);
    assert_eq!(resp.reading_status.unread, 0);
    assert!(resp.by_format.is_empty());
    assert!(resp.by_language.is_empty());
    assert_eq!(resp.metadata.total_series, 0);
    assert!(resp.metadata.by_provider.is_empty());
    assert!(resp.currently_reading.is_empty());
    assert!(resp.recently_read.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_overview_matches_stats_for_same_data(pool: PgPool) {
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let user = create_user(&pool, "alice").await;

    let b1 = create_simple_book(&pool, library, Some(series), "B1", 100).await;
    let b2 = create_simple_book(&pool, library, Some(series), "B2", 50).await;
    let now = Utc::now();
    insert_file(&pool, b1, "cbz", 1234, now).await;
    set_book_language(&pool, b2, Some("fr")).await;
    set_book_summary_isbn(&pool, b1, Some("sum"), None).await;
    create_link(&pool, library, series, "comicvine", "approved").await;
    set_progress(&pool, b1, user, "read", None, Some(now)).await;
    set_progress(&pool, b2, user, "reading", Some(7), None).await;

    let state = test_state(pool.clone());
    let Json(full) = get_stats(State(state.clone()), stats_query(None), None)
        .await
        .unwrap();
    let Json(overview) = get_stats_overview(State(state), None).await.unwrap();

    assert_overview_eq(&full.overview, &overview.overview);
    assert_eq!(full.reading_status.unread, overview.reading_status.unread);
    assert_eq!(full.reading_status.reading, overview.reading_status.reading);
    assert_eq!(full.reading_status.read, overview.reading_status.read);
    assert_eq!(formats(&full.by_format), formats(&overview.by_format));
    assert_eq!(
        languages(&full.by_language),
        languages(&overview.by_language)
    );
    assert_eq!(full.metadata.total_series, overview.metadata.total_series);
    assert_eq!(full.metadata.series_linked, overview.metadata.series_linked);
    assert_eq!(
        full.metadata.books_with_summary,
        overview.metadata.books_with_summary
    );
    assert_eq!(providers(&full.metadata.by_provider).len(), 1);
    assert_eq!(
        providers(&overview.metadata.by_provider),
        providers(&full.metadata.by_provider)
    );

    assert_eq!(
        full.currently_reading.len(),
        overview.currently_reading.len()
    );
    assert_eq!(
        full.currently_reading[0].book_id,
        overview.currently_reading[0].book_id
    );
    assert_eq!(full.recently_read.len(), overview.recently_read.len());
    assert_eq!(
        full.recently_read[0].book_id,
        overview.recently_read[0].book_id
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_overview_scopes_reading_sections_to_user(pool: PgPool) {
    let library = create_library(&pool, "main").await;
    let alice = create_user(&pool, "alice").await;
    let bob = create_user(&pool, "bob").await;

    let b1 = create_simple_book(&pool, library, None, "B1", 10).await;
    let b2 = create_simple_book(&pool, library, None, "B2", 10).await;
    set_progress(&pool, b1, alice, "read", None, Some(Utc::now())).await;
    set_progress(&pool, b2, bob, "read", None, Some(Utc::now())).await;

    let state = test_state(pool.clone());
    let Json(scoped) = get_stats_overview(State(state.clone()), auth_user(alice))
        .await
        .unwrap();
    assert_eq!(scoped.reading_status.read, 1);
    assert_eq!(scoped.reading_status.unread, 1);
    assert_eq!(scoped.recently_read.len(), 1);
    assert_eq!(scoped.recently_read[0].book_id, b1.to_string());

    let Json(unscoped) = get_stats_overview(State(state), auth_user(bob))
        .await
        .unwrap();
    assert_eq!(unscoped.reading_status.read, 1);
    assert_eq!(unscoped.recently_read[0].book_id, b2.to_string());
}

// ---------------------------------------------------------------------------
// get_stats_breakdown
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_breakdown_empty_db(pool: PgPool) {
    let state = test_state(pool.clone());

    let Json(resp) = get_stats_breakdown(State(state), None).await.unwrap();

    assert!(resp.by_library.is_empty());
    assert!(resp.top_series.is_empty());
    assert_eq!(resp.downloads.total_downloads, 0);
    assert_eq!(resp.downloads.available_series, 0);
    assert!(resp.downloads.recent_downloads.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_breakdown_returns_libraries_series_and_downloads(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;

    let b1 = create_simple_book(&pool, library, Some(series), "B1", 10).await;
    let _b2 = create_simple_book(&pool, library, Some(series), "B2", 20).await;
    insert_file(&pool, b1, "cbz", 512, Utc::now()).await;
    create_torrent(&pool, library, "T1", "imported", vec![1], Utc::now()).await;
    create_available(&pool, library, series, 4).await;

    let Json(resp) = get_stats_breakdown(State(state), None).await.unwrap();

    assert_eq!(resp.by_library.len(), 1);
    assert_eq!(resp.by_library[0].library_name, "main");
    assert_eq!(resp.by_library[0].book_count, 2);
    assert_eq!(resp.by_library[0].size_bytes, 512);

    assert_eq!(resp.top_series.len(), 1);
    assert_eq!(resp.top_series[0].series, "S1");
    assert_eq!(resp.top_series[0].book_count, 2);

    assert_eq!(resp.downloads.imported_downloads, 1);
    assert_eq!(resp.downloads.total_downloads, 1);
    assert_eq!(resp.downloads.available_series, 1);
    assert_eq!(resp.downloads.total_missing_volumes, 4);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_stats_breakdown_scopes_library_and_series_counts_to_user(pool: PgPool) {
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1").await;
    let alice = create_user(&pool, "alice").await;
    let bob = create_user(&pool, "bob").await;

    let b1 = create_simple_book(&pool, library, Some(series), "B1", 10).await;
    let b2 = create_simple_book(&pool, library, Some(series), "B2", 10).await;
    set_progress(&pool, b1, alice, "read", None, Some(Utc::now())).await;
    set_progress(&pool, b2, bob, "read", None, Some(Utc::now())).await;

    let state = test_state(pool.clone());
    let Json(scoped) = get_stats_breakdown(State(state), auth_user(alice))
        .await
        .unwrap();

    assert_eq!(scoped.by_library[0].read_count, 1);
    assert_eq!(scoped.by_library[0].unread_count, 1);
    assert_eq!(scoped.top_series[0].read_count, 1);
    assert_eq!(scoped.top_series[0].book_count, 2);
}

// ---------------------------------------------------------------------------
// get_reading_overview
// ---------------------------------------------------------------------------

fn find_series<'a>(
    items: &'a [UserReadingOverviewSeries],
    name: &str,
) -> &'a UserReadingOverviewSeries {
    items
        .iter()
        .find(|s| s.series_name == name)
        .unwrap_or_else(|| panic!("series {name} not found"))
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_overview_empty_db_returns_no_users(pool: PgPool) {
    let state = test_state(pool);

    let Json(resp) = get_reading_overview(State(state)).await.unwrap();

    assert!(resp.is_empty());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_overview_aggregates_status_and_series_progress(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let s1 = create_series(&pool, library, "S1").await;
    let s2 = create_series(&pool, library, "S2").await;
    let alice = create_user(&pool, "alice").await;
    let bob = create_user(&pool, "bob").await;

    let b1 = create_simple_book(&pool, library, Some(s1), "B1", 10).await;
    let b2 = create_simple_book(&pool, library, Some(s1), "B2", 20).await;
    let b3 = create_simple_book(&pool, library, Some(s2), "B3", 30).await;
    let b4 = create_simple_book(&pool, library, None, "B4", 40).await;

    let now = Utc::now();
    set_progress(
        &pool,
        b1,
        alice,
        "read",
        None,
        Some(now - Duration::hours(1)),
    )
    .await;
    set_progress(&pool, b2, alice, "reading", Some(3), None).await;
    set_progress(&pool, b3, bob, "read", None, Some(now - Duration::hours(2))).await;

    let Json(resp) = get_reading_overview(State(state)).await.unwrap();

    assert_eq!(resp.len(), 2);
    assert_eq!(resp[0].username, "alice");
    assert_eq!(resp[1].username, "bob");

    let a = &resp[0];
    assert_eq!(a.books_read, 1);
    assert_eq!(a.books_reading, 1);
    assert_eq!(a.series_in_progress, 1);
    assert!(a.last_read_at.is_some());
    assert_eq!(a.currently_reading.len(), 1);
    assert_eq!(a.currently_reading[0].book_id, b2.to_string());
    assert_eq!(a.currently_reading[0].current_page, 3);
    assert_eq!(a.recently_read.len(), 1);
    assert_eq!(a.recently_read[0].book_id, b1.to_string());

    assert_eq!(a.series_progress.len(), 3);
    let a_s1 = find_series(&a.series_progress, "S1");
    assert_eq!(a_s1.books_total, 2);
    assert_eq!(a_s1.books_read, 1);
    assert_eq!(a_s1.books_reading, 1);
    assert_eq!(a_s1.books_unread, 0);
    assert_eq!(a_s1.books.len(), 2);
    assert!(a_s1.series_id.is_some());

    let a_s2 = find_series(&a.series_progress, "S2");
    assert_eq!(a_s2.books_total, 1);
    assert_eq!(a_s2.books_unread, 1);

    let a_orphan = find_series(&a.series_progress, "Sans série");
    assert_eq!(a_orphan.books_total, 1);
    assert_eq!(a_orphan.books_unread, 1);
    assert!(a_orphan.series_id.is_none());
    assert_eq!(a_orphan.books[0].book_id, b4.to_string());

    let b = &resp[1];
    assert_eq!(b.books_read, 1);
    assert_eq!(b.books_reading, 0);
    assert_eq!(b.series_in_progress, 0);
    assert!(b.currently_reading.is_empty());
    assert_eq!(b.recently_read.len(), 1);
    assert_eq!(b.recently_read[0].book_id, b3.to_string());

    let b_s1 = find_series(&b.series_progress, "S1");
    assert_eq!(b_s1.books_total, 2);
    assert_eq!(b_s1.books_read, 0);
    assert_eq!(b_s1.books_unread, 2);
    let b_s2 = find_series(&b.series_progress, "S2");
    assert_eq!(b_s2.books_read, 1);
    assert_eq!(b_s2.books_unread, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_overview_includes_users_without_progress(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let s1 = create_series(&pool, library, "S1").await;
    let _carol = create_user(&pool, "carol").await;
    let b1 = create_simple_book(&pool, library, Some(s1), "B1", 10).await;

    let Json(resp) = get_reading_overview(State(state)).await.unwrap();

    assert_eq!(resp.len(), 1);
    let carol = &resp[0];
    assert_eq!(carol.username, "carol");
    assert_eq!(carol.books_read, 0);
    assert_eq!(carol.books_reading, 0);
    assert_eq!(carol.series_in_progress, 0);
    assert!(carol.last_read_at.is_none());
    assert!(carol.currently_reading.is_empty());
    assert!(carol.recently_read.is_empty());

    assert_eq!(carol.series_progress.len(), 1);
    let s1_progress = find_series(&carol.series_progress, "S1");
    assert_eq!(s1_progress.books_total, 1);
    assert_eq!(s1_progress.books_unread, 1);
    assert_eq!(s1_progress.books[0].book_id, b1.to_string());
    assert_eq!(s1_progress.books[0].status, "unread");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn get_reading_overview_series_progress_is_per_user(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let s1 = create_series(&pool, library, "S1").await;
    let alice = create_user(&pool, "alice").await;
    let bob = create_user(&pool, "bob").await;

    let b1 = create_simple_book(&pool, library, Some(s1), "B1", 10).await;
    let b2 = create_simple_book(&pool, library, Some(s1), "B2", 20).await;

    set_progress(&pool, b1, alice, "read", None, None).await;
    set_progress(&pool, b2, alice, "reading", Some(5), None).await;
    set_progress(&pool, b1, bob, "reading", Some(2), None).await;

    let Json(resp) = get_reading_overview(State(state)).await.unwrap();

    let a = resp.iter().find(|u| u.username == "alice").unwrap();
    let a_s1 = find_series(&a.series_progress, "S1");
    assert_eq!(a_s1.books_total, 2);
    assert_eq!(a_s1.books_read, 1);
    assert_eq!(a_s1.books_reading, 1);
    assert_eq!(a_s1.books_unread, 0);

    let b = resp.iter().find(|u| u.username == "bob").unwrap();
    let b_s1 = find_series(&b.series_progress, "S1");
    assert_eq!(b_s1.books_total, 2);
    assert_eq!(b_s1.books_read, 0);
    assert_eq!(b_s1.books_reading, 1);
    assert_eq!(b_s1.books_unread, 1);
    let b_b1 = b_s1
        .books
        .iter()
        .find(|bk| bk.book_id == b1.to_string())
        .unwrap();
    assert_eq!(b_b1.status, "reading");
    assert_eq!(b_b1.current_page, 2);
    let b_b2 = b_s1
        .books
        .iter()
        .find(|bk| bk.book_id == b2.to_string())
        .unwrap();
    assert_eq!(b_b2.status, "unread");
}
