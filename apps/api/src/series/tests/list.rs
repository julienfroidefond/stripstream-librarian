//! Integration tests for series listing (`list_series`, `list_all_series`).
//!
//! Handlers are called directly with a hand-built [`AppState`] backed by an
//! ephemeral Postgres provisioned by `#[sqlx::test]`. These lock the pagination
//! and `total` semantics after the count/data queries were merged into one.

use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};

use axum::extract::{Path, Query, State};
use axum::Json;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};
use uuid::Uuid;

use super::super::{list_all_series, list_series, ListAllSeriesQuery, ListSeriesQuery};
use crate::state::{
    DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};
use crate::AppState;

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

fn series_query(page: Option<i64>, limit: Option<i64>) -> Query<ListSeriesQuery> {
    Query(ListSeriesQuery {
        q: None,
        reading_status: None,
        series_status: None,
        has_missing: None,
        metadata_provider: None,
        has_books: None,
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
