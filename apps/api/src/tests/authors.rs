//! Integration tests for the `authors` module.
//!
//! Handlers are called directly with a hand-built [`AppState`] backed by an
//! ephemeral Postgres provisioned by `#[sqlx::test]`. This exercises the real
//! aggregation SQL (book-level and series-level authors, ILIKE search, sorting,
//! windowed total and pagination) without starting an HTTP server.
//!
//! No production fix is included: every assertion locks current behaviour.

use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};

use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};
use uuid::Uuid;

use super::*;
use crate::state::{
    DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
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
        read_rate_limit: Arc::new(std::sync::Mutex::new(ReadRateLimit::new())),
        stats_cache: Arc::new(crate::stats::StatsCache::new()),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
    }
}

fn query(
    q: Option<&str>,
    page: Option<i64>,
    limit: Option<i64>,
    sort: Option<&str>,
) -> Query<ListAuthorsQuery> {
    Query(ListAuthorsQuery {
        q: q.map(str::to_string),
        page,
        limit,
        sort: sort.map(str::to_string),
    })
}

fn names(response: &AuthorsPageResponse) -> Vec<&str> {
    response.items.iter().map(|i| i.name.as_str()).collect()
}

fn find<'a>(response: &'a AuthorsPageResponse, name: &str) -> &'a AuthorItem {
    response
        .items
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("author {name} not found"))
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

async fn create_series(pool: &PgPool, library_id: Uuid, name: &str, authors: &[&str]) -> Uuid {
    let authors: Vec<String> = authors.iter().map(|a| a.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, authors) \
         VALUES (gen_random_uuid(), $1, $2, $3) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(authors)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn create_book(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Option<Uuid>,
    title: &str,
    authors: &[&str],
    author: Option<&str>,
) -> Uuid {
    let authors: Vec<String> = authors.iter().map(|a| a.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO books \
             (id, library_id, kind, title, series_id, volume, volume_type, authors, author, page_count) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, NULL, 'regular', $4, $5, 10) RETURNING id",
    )
    .bind(library_id)
    .bind(title)
    .bind(series_id)
    .bind(authors)
    .bind(author)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// list_authors — aggregation
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_empty_database_returns_empty_page(pool: PgPool) {
    let state = test_state(pool);

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    assert!(response.items.is_empty());
    assert_eq!(response.total, 0);
    assert_eq!(response.page, 1);
    assert_eq!(response.limit, 20);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_counts_books_and_series(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1", &[]).await;
    create_book(&pool, library, Some(series), "v1", &["Urasawa"], None).await;
    create_book(&pool, library, Some(series), "v2", &["Urasawa"], None).await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    let author = find(&response, "Urasawa");
    assert_eq!(author.book_count, 2);
    assert_eq!(author.series_count, 1);
    assert_eq!(response.total, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_includes_series_level_authors_without_books(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_series(&pool, library, "Asterix", &["Goscinny"]).await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    let author = find(&response, "Goscinny");
    assert_eq!(author.book_count, 0);
    assert_eq!(author.series_count, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_falls_back_to_single_author_column(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "solo", &[], Some("Moebius")).await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    let author = find(&response, "Moebius");
    assert_eq!(author.book_count, 1);
    assert_eq!(author.series_count, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_prefers_authors_array_over_author_column(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(
        &pool,
        library,
        None,
        "both",
        &["ArrayName"],
        Some("ColumnName"),
    )
    .await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["ArrayName"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_ignores_empty_and_null_names(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "blank", &[""], None).await;
    create_book(&pool, library, None, "null", &[], None).await;
    create_book(&pool, library, None, "real", &["Real"], None).await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["Real"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_excludes_whitespace_only_names(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "blank", &["  "], None).await;
    create_book(&pool, library, None, "real", &["Real"], None).await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["Real"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_deduplicates_series_count_across_books(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    let first = create_series(&pool, library, "S1", &[]).await;
    let second = create_series(&pool, library, "S2", &[]).await;
    create_book(&pool, library, Some(first), "a", &["Shared"], None).await;
    create_book(&pool, library, Some(first), "b", &["Shared"], None).await;
    create_book(&pool, library, Some(second), "c", &["Shared"], None).await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    let author = find(&response, "Shared");
    assert_eq!(author.book_count, 3);
    assert_eq!(author.series_count, 2);
}

// ---------------------------------------------------------------------------
// list_authors — search, sort, pagination
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_search_is_case_insensitive_substring(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "a", &["Naoki Urasawa"], None).await;
    create_book(&pool, library, None, "b", &["Osamu Tezuka"], None).await;

    let Json(response) = list_authors(State(state), query(Some("URASAWA"), None, None, None))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["Naoki Urasawa"]);
    assert_eq!(response.total, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_blank_search_is_ignored(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "a", &["Alpha"], None).await;
    create_book(&pool, library, None, "b", &["Beta"], None).await;

    let Json(response) = list_authors(State(state), query(Some("   "), None, None, None))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["Alpha", "Beta"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_default_sort_is_name_ascending(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "a", &["Zola"], None).await;
    create_book(&pool, library, None, "b", &["Alpha"], None).await;
    create_book(&pool, library, None, "c", &["Mike"], None).await;

    let Json(response) = list_authors(State(state), query(None, None, None, None))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["Alpha", "Mike", "Zola"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_sort_books_orders_by_count_then_name(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "a", &["Zola"], None).await;
    create_book(&pool, library, None, "b", &["Alpha"], None).await;
    create_book(&pool, library, None, "c", &["Alpha"], None).await;
    create_book(&pool, library, None, "d", &["Mike"], None).await;

    let Json(response) = list_authors(State(state), query(None, None, None, Some("books")))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["Alpha", "Mike", "Zola"]);
    assert_eq!(find(&response, "Alpha").book_count, 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_unknown_sort_falls_back_to_name(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "a", &["Zola"], None).await;
    create_book(&pool, library, None, "b", &["Alpha"], None).await;

    let Json(response) = list_authors(State(state), query(None, None, None, Some("bogus")))
        .await
        .unwrap();

    assert_eq!(names(&response), vec!["Alpha", "Zola"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_paginates_and_reports_windowed_total(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    for name in ["Alpha", "Beta", "Gamma", "Delta"] {
        create_book(&pool, library, None, name, &[name], None).await;
    }

    let Json(first) = list_authors(State(state.clone()), query(None, Some(1), Some(2), None))
        .await
        .unwrap();
    assert_eq!(names(&first), vec!["Alpha", "Beta"]);
    assert_eq!(first.total, 4);
    assert_eq!(first.page, 1);
    assert_eq!(first.limit, 2);

    let Json(second) = list_authors(State(state), query(None, Some(2), Some(2), None))
        .await
        .unwrap();
    assert_eq!(names(&second), vec!["Delta", "Gamma"]);
    assert_eq!(second.total, 4);
    assert_eq!(second.page, 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_page_below_one_is_clamped(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "a", &["Alpha"], None).await;

    let Json(response) = list_authors(State(state), query(None, Some(0), None, None))
        .await
        .unwrap();

    assert_eq!(response.page, 1);
    assert_eq!(names(&response), vec!["Alpha"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_limit_is_clamped_to_100(pool: PgPool) {
    let state = test_state(pool);

    let Json(response) = list_authors(State(state), query(None, None, Some(500), None))
        .await
        .unwrap();

    assert_eq!(response.limit, 100);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_limit_below_one_is_clamped(pool: PgPool) {
    let state = test_state(pool);

    let Json(response) = list_authors(State(state), query(None, None, Some(0), None))
        .await
        .unwrap();

    assert_eq!(response.limit, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn list_authors_out_of_range_page_returns_empty_with_total(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;
    create_book(&pool, library, None, "a", &["Alpha"], None).await;

    let Json(response) = list_authors(State(state), query(None, Some(5), Some(20), None))
        .await
        .unwrap();

    assert!(response.items.is_empty());
    assert_eq!(response.total, 1);
}
