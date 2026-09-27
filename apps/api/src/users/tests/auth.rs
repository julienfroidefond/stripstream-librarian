use super::*;
use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};

use axum::body::Body;
use axum::http::{header::AUTHORIZATION, Request as HttpRequest, StatusCode};
use axum::middleware;
use axum::routing::get;
use axum::Router;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tower::ServiceExt;

use crate::state::{
    DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};

// -----------------------------------------------------------------------
// Harness
// -----------------------------------------------------------------------

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

async fn insert_user(pool: &PgPool, username: &str) -> uuid::Uuid {
    let id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username) VALUES ($1, $2)")
        .bind(id)
        .bind(username)
        .execute(pool)
        .await
        .unwrap();
    id
}

fn admin_router(state: AppState) -> Router {
    Router::new()
        .route("/protected", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(state, require_admin))
}

fn read_router(state: AppState) -> Router {
    Router::new()
        .route("/protected", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(state, require_read))
}

async fn send_as(router: Router, token: &str, as_user: Option<&str>) -> StatusCode {
    let mut request = HttpRequest::builder()
        .uri("/protected")
        .header(AUTHORIZATION, format!("Bearer {token}"));
    if let Some(value) = as_user {
        request = request.header("X-As-User", value);
    }
    router
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

// -----------------------------------------------------------------------
// parse_prefix
// -----------------------------------------------------------------------

#[test]
fn parse_prefix_valid_token() {
    let token = "stl_abcdefgh_somesecretvalue";
    assert_eq!(parse_prefix(token), Some("abcdefgh"));
}

#[test]
fn parse_prefix_exact_minimum_length() {
    // 8-char prefix + '_' + 1-char secret = 10 chars after "stl_"
    let token = "stl_12345678_x";
    assert_eq!(parse_prefix(token), Some("12345678"));
}

#[test]
fn parse_prefix_missing_stl_prefix() {
    assert_eq!(parse_prefix("abc_12345678_secret"), None);
}

#[test]
fn parse_prefix_empty_string() {
    assert_eq!(parse_prefix(""), None);
}

#[test]
fn parse_prefix_only_stl_prefix() {
    assert_eq!(parse_prefix("stl_"), None);
}

#[test]
fn parse_prefix_too_short_after_stl() {
    // 9 chars after "stl_" — needs at least 10
    assert_eq!(parse_prefix("stl_12345678"), None);
}

#[test]
fn parse_prefix_no_separator_after_prefix() {
    // 8 chars + a non-underscore char at position 8
    assert_eq!(parse_prefix("stl_12345678Xsecret"), None);
}

#[test]
fn parse_prefix_with_underscores_in_secret() {
    // Base64 URL_SAFE can contain underscores — prefix should still be first 8 chars
    let token = "stl_ABCD_fgh_secret_with_underscores";
    assert_eq!(parse_prefix(token), Some("ABCD_fgh"));
}

#[test]
fn parse_prefix_long_secret() {
    let token = "stl_PREFIXab_aVeryLongSecretValueThatCouldBeBase64Encoded";
    assert_eq!(parse_prefix(token), Some("PREFIXab"));
}

// -----------------------------------------------------------------------
// bearer_token
// -----------------------------------------------------------------------

#[test]
fn bearer_token_extracts_correctly() {
    let req = HttpRequest::builder()
        .header(AUTHORIZATION, "Bearer my_token_here")
        .body(())
        .unwrap();
    // Convert to axum Request type
    let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
    assert_eq!(bearer_token(&req), Some("my_token_here"));
}

#[test]
fn bearer_token_missing_header() {
    let req = HttpRequest::builder().body(()).unwrap();
    let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
    assert_eq!(bearer_token(&req), None);
}

#[test]
fn bearer_token_wrong_scheme() {
    let req = HttpRequest::builder()
        .header(AUTHORIZATION, "Basic abc123")
        .body(())
        .unwrap();
    let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
    assert_eq!(bearer_token(&req), None);
}

#[test]
fn bearer_token_lowercase_scheme_is_accepted() {
    let req = HttpRequest::builder()
        .header(AUTHORIZATION, "bearer my_token")
        .body(())
        .unwrap();
    let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
    assert_eq!(bearer_token(&req), Some("my_token"));
}

#[test]
fn bearer_token_empty_token_value() {
    let req = HttpRequest::builder()
        .header(AUTHORIZATION, "Bearer ")
        .body(())
        .unwrap();
    let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
    assert_eq!(bearer_token(&req), Some(""));
}

// -----------------------------------------------------------------------
// Scope enum
// -----------------------------------------------------------------------

#[test]
fn scope_admin_matches_correctly() {
    let scope = Scope::Admin;
    assert!(matches!(scope, Scope::Admin));
}

#[test]
fn scope_read_does_not_match_admin() {
    let scope = Scope::Read {
        user_id: uuid::Uuid::nil(),
    };
    assert!(!matches!(scope, Scope::Admin));
}

#[test]
fn scope_read_carries_user_id() {
    let id = uuid::Uuid::new_v4();
    let scope = Scope::Read { user_id: id };
    if let Scope::Read { user_id } = scope {
        assert_eq!(user_id, id);
    } else {
        panic!("expected Scope::Read");
    }
}

// -----------------------------------------------------------------------
// constant_time_eq
// -----------------------------------------------------------------------

#[test]
fn constant_time_eq_true_for_identical_values() {
    assert!(constant_time_eq(b"bootstrap-secret", b"bootstrap-secret"));
}

#[test]
fn constant_time_eq_false_for_same_length_different_values() {
    assert!(!constant_time_eq(b"bootstrap-secret", b"bootstrap-secreT"));
}

#[test]
fn constant_time_eq_false_for_single_differing_byte() {
    assert!(!constant_time_eq(b"abcdefgh", b"abcdEfgh"));
}

#[test]
fn constant_time_eq_false_for_different_lengths() {
    assert!(!constant_time_eq(b"secret", b"secret-longer"));
}

#[test]
fn constant_time_eq_true_for_empty_slices() {
    assert!(constant_time_eq(b"", b""));
}

#[test]
fn constant_time_eq_false_when_one_side_is_empty() {
    assert!(!constant_time_eq(b"", b"x"));
}

// -----------------------------------------------------------------------
// requested_impersonation
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn requested_impersonation_absent_header_returns_none(pool: PgPool) {
    let state = test_state(pool);

    let resolved = requested_impersonation(&state, None).await.unwrap();

    assert_eq!(resolved, None);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn requested_impersonation_existing_user_returns_id(pool: PgPool) {
    let user_id = insert_user(&pool, "impersonated").await;
    let state = test_state(pool);

    let resolved = requested_impersonation(&state, Some(user_id.to_string()))
        .await
        .unwrap();

    assert_eq!(resolved, Some(user_id));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn requested_impersonation_malformed_uuid_is_bad_request(pool: PgPool) {
    let state = test_state(pool);

    let err = requested_impersonation(&state, Some("not-a-uuid".to_string()))
        .await
        .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn requested_impersonation_unknown_user_is_bad_request(pool: PgPool) {
    let state = test_state(pool);

    let err = requested_impersonation(&state, Some(uuid::Uuid::new_v4().to_string()))
        .await
        .unwrap_err();

    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}

// -----------------------------------------------------------------------
// require_admin / require_read impersonation
// -----------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn require_admin_allows_valid_impersonation(pool: PgPool) {
    let user_id = insert_user(&pool, "target-user").await;
    let router = admin_router(test_state(pool));

    let status = send_as(router, "test-token", Some(&user_id.to_string())).await;

    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn require_admin_without_impersonation_succeeds(pool: PgPool) {
    let router = admin_router(test_state(pool));

    let status = send_as(router, "test-token", None).await;

    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn require_admin_rejects_malformed_impersonation(pool: PgPool) {
    let router = admin_router(test_state(pool));

    let status = send_as(router, "test-token", Some("12345")).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn require_admin_rejects_unknown_impersonation(pool: PgPool) {
    let router = admin_router(test_state(pool));

    let status = send_as(
        router,
        "test-token",
        Some(&uuid::Uuid::new_v4().to_string()),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn require_read_rejects_unknown_impersonation_for_admin(pool: PgPool) {
    let router = read_router(test_state(pool));

    let status = send_as(
        router,
        "test-token",
        Some(&uuid::Uuid::new_v4().to_string()),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}
