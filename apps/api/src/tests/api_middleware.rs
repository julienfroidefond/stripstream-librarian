//! Integration tests for the `api_middleware` module.
//!
//! The middlewares are exercised through a real axum router built with
//! `middleware::from_fn_with_state`, so the actual `Next` chain runs. The
//! `AppState` is backed by an ephemeral Postgres provisioned by `#[sqlx::test]`.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware;
use axum::routing::get;
use axum::Router;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tower::ServiceExt;

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
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit::new())),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
    }
}

fn counter_router(state: AppState) -> Router {
    Router::new()
        .route("/ping", get(|| async { "pong" }))
        .layer(middleware::from_fn_with_state(state, request_counter))
}

fn rate_limit_router(state: AppState) -> Router {
    Router::new()
        .route("/read", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(state, read_rate_limit))
}

async fn send(router: Router, path: &str) -> StatusCode {
    router
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

async fn send_as(router: Router, path: &str, token: &str) -> StatusCode {
    router
        .oneshot(
            Request::builder()
                .uri(path)
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

async fn set_rate_limit(state: &AppState, value: u32) {
    state.settings.write().await.rate_limit_per_second = value;
}

// ---------------------------------------------------------------------------
// request_counter
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn request_counter_increments_on_each_request(pool: PgPool) {
    let state = test_state(pool);
    let router = counter_router(state.clone());

    assert_eq!(send(router.clone(), "/ping").await, StatusCode::OK);
    assert_eq!(send(router.clone(), "/ping").await, StatusCode::OK);
    assert_eq!(send(router, "/ping").await, StatusCode::OK);

    assert_eq!(state.metrics.requests_total.load(Ordering::Relaxed), 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn request_counter_passes_response_through(pool: PgPool) {
    let state = test_state(pool);
    let router = counter_router(state);

    let response = router
        .oneshot(Request::builder().uri("/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"pong");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn request_counter_counts_not_found_responses(pool: PgPool) {
    let state = test_state(pool);
    let router = counter_router(state.clone());

    assert_eq!(send(router, "/missing").await, StatusCode::NOT_FOUND);

    assert_eq!(state.metrics.requests_total.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------------------
// read_rate_limit
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn read_rate_limit_allows_requests_up_to_limit(pool: PgPool) {
    let state = test_state(pool);
    set_rate_limit(&state, 3).await;
    let router = rate_limit_router(state);

    assert_eq!(send(router.clone(), "/read").await, StatusCode::OK);
    assert_eq!(send(router.clone(), "/read").await, StatusCode::OK);
    assert_eq!(send(router, "/read").await, StatusCode::OK);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn read_rate_limit_rejects_request_over_limit(pool: PgPool) {
    let state = test_state(pool);
    set_rate_limit(&state, 2).await;
    let router = rate_limit_router(state);

    assert_eq!(send(router.clone(), "/read").await, StatusCode::OK);
    assert_eq!(send(router.clone(), "/read").await, StatusCode::OK);
    assert_eq!(send(router, "/read").await, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn read_rate_limit_zero_blocks_every_request(pool: PgPool) {
    let state = test_state(pool);
    set_rate_limit(&state, 0).await;
    let router = rate_limit_router(state);

    assert_eq!(send(router, "/read").await, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn read_rate_limit_is_per_client(pool: PgPool) {
    let state = test_state(pool);
    set_rate_limit(&state, 1).await;
    let router = rate_limit_router(state);

    assert_eq!(
        send_as(router.clone(), "/read", "client-a").await,
        StatusCode::OK
    );
    assert_eq!(
        send_as(router.clone(), "/read", "client-a").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(send_as(router, "/read", "client-b").await, StatusCode::OK);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn read_rate_limit_resets_window_after_one_second(pool: PgPool) {
    let state = test_state(pool);
    set_rate_limit(&state, 1).await;
    let router = rate_limit_router(state.clone());

    assert_eq!(send(router.clone(), "/read").await, StatusCode::OK);
    assert_eq!(
        send(router.clone(), "/read").await,
        StatusCode::TOO_MANY_REQUESTS
    );

    state
        .read_rate_limit
        .lock()
        .await
        .expire_all(Duration::from_secs(2));

    assert_eq!(send(router, "/read").await, StatusCode::OK);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn read_rate_limit_rejection_is_json_with_retry_after(pool: PgPool) {
    let state = test_state(pool);
    set_rate_limit(&state, 0).await;
    let router = rate_limit_router(state);

    let response = router
        .oneshot(Request::builder().uri("/read").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response.headers().get("retry-after").unwrap(),
        axum::http::HeaderValue::from_static("1")
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "rate limit exceeded");
}
