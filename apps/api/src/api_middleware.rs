use axum::{
    extract::State,
    http::{
        header::{AUTHORIZATION, RETRY_AFTER},
        HeaderValue, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::atomic::Ordering;
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
    let key = client_key(&req);
    let rate_limit = state.settings.read().await.rate_limit_per_second;

    let allowed = state
        .read_rate_limit
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .check(&key, rate_limit);
    if !allowed {
        return rate_limited();
    }

    next.run(req).await
}

fn client_key(req: &axum::extract::Request) -> String {
    use std::hash::{Hash, Hasher};

    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("anonymous");

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn rate_limited() -> Response {
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(serde_json::json!({ "error": "rate limit exceeded" })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from_static("1"));
    response
}

#[cfg(test)]
#[path = "tests/api_middleware.rs"]
mod tests;
