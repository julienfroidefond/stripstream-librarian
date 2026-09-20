//! Integration tests for the `ai_tagging` module.
//!
//! Handlers are called directly with a hand-built [`AppState`] backed by an
//! ephemeral Postgres provisioned by `#[sqlx::test]`. Only the paths that do not
//! reach an external AI provider are exercised: request validation, the
//! `ai_tagging` setting gate, and the `test_connection` input validation.
//!
//! No production fix is included: every assertion locks current behaviour.

use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};
use std::time::Instant;

use axum::http::StatusCode;
use serde_json::{json, Value};
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
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit {
            window_started_at: Instant::now(),
            requests_in_window: 0,
        })),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
    }
}

#[allow(dead_code)]
fn expect_err<T>(result: Result<T, ApiError>) -> ApiError {
    match result {
        Ok(_) => panic!("expected an error result"),
        Err(err) => err,
    }
}

fn suggest_body(series_ids: Vec<Uuid>) -> Json<SuggestTagsRequest> {
    Json(SuggestTagsRequest { series_ids })
}

fn connection_body(base_url: &str, api_key: &str, model: &str) -> Json<TestConnectionRequest> {
    Json(TestConnectionRequest {
        base_url: base_url.to_string(),
        api_key: api_key.to_string(),
        model: model.to_string(),
    })
}

async fn set_ai_tagging_setting(pool: &PgPool, value: Value) {
    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('ai_tagging', $1) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(value)
    .execute(pool)
    .await
    .unwrap();
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

async fn create_series(pool: &PgPool, library_id: Uuid, name: &str, genres: &[&str]) -> Uuid {
    let genres: Vec<String> = genres.iter().map(|g| g.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO series (id, library_id, name, genres) \
         VALUES (gen_random_uuid(), $1, $2, $3) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(genres)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// suggest_tags — request validation
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_rejects_empty_series_ids(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(suggest_tags(State(state), suggest_body(vec![])).await);

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.message, "at least one series is required");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_rejects_more_than_25_series(pool: PgPool) {
    let state = test_state(pool);
    let ids: Vec<Uuid> = (0..26).map(|_| Uuid::new_v4()).collect();

    let err = expect_err(suggest_tags(State(state), suggest_body(ids)).await);

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.message, "at most 25 series can be analyzed at once");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_accepts_exactly_25_series(pool: PgPool) {
    let state = test_state(pool);
    let ids: Vec<Uuid> = (0..25).map(|_| Uuid::new_v4()).collect();

    let err = expect_err(suggest_tags(State(state), suggest_body(ids)).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "AI tagging is disabled");
}

// ---------------------------------------------------------------------------
// suggest_tags — configuration gate
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_disabled_when_setting_missing(pool: PgPool) {
    let state = test_state(pool);

    let err = expect_err(suggest_tags(State(state), suggest_body(vec![Uuid::new_v4()])).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "AI tagging is disabled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_disabled_when_enabled_false(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_tagging_setting(&pool, json!({ "enabled": false, "api_key": "sk-test" })).await;

    let err = expect_err(suggest_tags(State(state), suggest_body(vec![Uuid::new_v4()])).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "AI tagging is disabled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_not_configured_when_api_key_missing(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_tagging_setting(&pool, json!({ "enabled": true })).await;

    let err = expect_err(suggest_tags(State(state), suggest_body(vec![Uuid::new_v4()])).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "AI tagging is not configured");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_not_configured_when_api_key_blank(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_tagging_setting(&pool, json!({ "enabled": true, "api_key": "   " })).await;

    let err = expect_err(suggest_tags(State(state), suggest_body(vec![Uuid::new_v4()])).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "AI tagging is not configured");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_requires_existing_genres(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_tagging_setting(&pool, json!({ "enabled": true, "api_key": "sk-test" })).await;
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1", &[]).await;

    let err = expect_err(suggest_tags(State(state), suggest_body(vec![series])).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "no existing genres are available");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn suggest_tags_ignores_blank_genres_when_checking_availability(pool: PgPool) {
    let state = test_state(pool.clone());
    set_ai_tagging_setting(&pool, json!({ "enabled": true, "api_key": "sk-test" })).await;
    let library = create_library(&pool, "main").await;
    let series = create_series(&pool, library, "S1", &["   "]).await;

    let err = expect_err(suggest_tags(State(state), suggest_body(vec![series])).await);

    assert_eq!(err.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err.message, "no existing genres are available");
}

// ---------------------------------------------------------------------------
// test_connection — input validation
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_connection_rejects_blank_api_key(pool: PgPool) {
    let _ = pool;

    let err =
        expect_err(test_connection(connection_body("https://example.com", "   ", "gpt-4")).await);

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.message, "API key is required");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_connection_rejects_blank_base_url(pool: PgPool) {
    let _ = pool;

    let err = expect_err(test_connection(connection_body("   ", "sk-test", "gpt-4")).await);

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.message, "API URL is required");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_connection_rejects_slash_only_base_url(pool: PgPool) {
    let _ = pool;

    let err = expect_err(test_connection(connection_body("///", "sk-test", "gpt-4")).await);

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(err.message, "API URL is required");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_connection_unreachable_provider_is_internal_error(pool: PgPool) {
    let _ = pool;

    let err = expect_err(
        test_connection(connection_body("http://127.0.0.1:1", "sk-test", "gpt-4")).await,
    );

    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(err.message.starts_with("HTTP client error:"));
}
