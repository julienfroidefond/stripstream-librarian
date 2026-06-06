use axum::{
    extract::State,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tracing::{debug, info};

use crate::state::AppState;

pub async fn request_counter(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let elapsed = start.elapsed();
    let path = uri.path();
    // High-frequency polling routes logged at debug to avoid log noise
    let noisy = matches!(
        path,
        "/health" | "/index/status" | "/torrent-downloads" | "/settings/downloads_enabled"
    );
    if noisy {
        debug!("{} {} {} {}ms", method, path, status, elapsed.as_millis());
    } else {
        info!("{} {} {} {}ms", method, path, status, elapsed.as_millis());
    }
    response
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

    let rate_limit = state.settings.read().await.rate_limit_per_second;
    if limiter.requests_in_window >= rate_limit {
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
