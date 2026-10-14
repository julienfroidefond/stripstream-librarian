//! Integration tests for the `handlers` module.
//!
//! Handlers are called directly with the shared `test_state` helper, backed by
//! an ephemeral Postgres provisioned by `#[sqlx::test]`. `ready` hits the real
//! database; the remaining handlers are pure and need no pool.
//!
//! No production fix is included: every assertion locks current behaviour.

use std::sync::atomic::Ordering;

use axum::response::IntoResponse;
use sqlx::PgPool;

use super::*;
use crate::state::test_state;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// health / version
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn health_returns_ok(pool: PgPool) {
    let _ = pool;

    assert_eq!(health().await, "ok");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn version_reports_crate_version(pool: PgPool) {
    let _ = pool;

    let Json(value) = version().await;

    assert_eq!(
        value,
        serde_json::json!({ "api": env!("CARGO_PKG_VERSION") })
    );
    assert!(value["api"].is_string());
}

// ---------------------------------------------------------------------------
// docs_redirect / api_home
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn docs_redirect_points_to_swagger_ui(pool: PgPool) {
    let _ = pool;

    let response = docs_redirect().await.into_response();

    assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::LOCATION)
            .unwrap(),
        "/swagger-ui/"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn api_home_renders_html_with_version_and_links(pool: PgPool) {
    let _ = pool;

    let response = api_home().await.into_response();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .unwrap(),
        "text/html; charset=utf-8"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Stripstream Librarian"));
    assert!(html.contains(env!("CARGO_PKG_VERSION")));
    assert!(html.contains("/swagger-ui/?urls.primaryName=%2Fopenapi.json"));
    assert!(html.contains("/swagger-ui/?urls.primaryName=%2Fadmin%2Fopenapi.json"));
    assert!(html.contains("/health"));
}

// ---------------------------------------------------------------------------
// ready
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn ready_reports_ready_when_database_reachable(pool: PgPool) {
    let state = test_state(pool);

    let Json(response) = ready(State(state)).await.unwrap();

    assert_eq!(response.status, "ready");
}

// ---------------------------------------------------------------------------
// metrics
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn metrics_renders_zeroed_counters(pool: PgPool) {
    let state = test_state(pool);

    let body = metrics(State(state)).await;

    assert_eq!(
        body,
        "requests_total 0\npage_cache_hits 0\npage_cache_misses 0\n"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn metrics_reflects_counter_values(pool: PgPool) {
    let state = test_state(pool);
    state.metrics.requests_total.store(12, Ordering::Relaxed);
    state.metrics.page_cache_hits.store(3, Ordering::Relaxed);
    state.metrics.page_cache_misses.store(4, Ordering::Relaxed);

    let body = metrics(State(state)).await;

    assert_eq!(
        body,
        "requests_total 12\npage_cache_hits 3\npage_cache_misses 4\n"
    );
}
