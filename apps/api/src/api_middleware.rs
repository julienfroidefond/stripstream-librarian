use axum::{
    extract::State,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Duration;
use std::sync::atomic::Ordering;

use crate::state::AppState;

pub async fn request_counter(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    next.run(req).await
}

pub async fn read_rate_limit(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let mut limiter = state.read_rate_limit.lock().await;
    if limiter.window_started_at.elapsed() >= Duration::from_secs(1) {
        limiter.window_started_at = std::time::Instant::now();
        limiter.requests_in_window = 0;
    }

    if limiter.requests_in_window >= 120 {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded",
        )
            .into_response();
    }

    limiter.requests_in_window += 1;
    drop(limiter);
    next.run(req).await
}
