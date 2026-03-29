use axum::{extract::State, Json};
use std::sync::atomic::Ordering;

use crate::{error::ApiError, state::AppState};

pub async fn health() -> &'static str {
    "ok"
}

pub async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "api": env!("CARGO_PKG_VERSION"),
    }))
}

pub async fn docs_redirect() -> impl axum::response::IntoResponse {
    axum::response::Redirect::to("/swagger-ui/")
}

pub async fn ready(State(state): State<AppState>) -> Result<Json<crate::responses::StatusResponse>, ApiError> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(Json(crate::responses::StatusResponse::new("ready")))
}

pub async fn metrics(State(state): State<AppState>) -> String {
    format!(
        "requests_total {}\npage_cache_hits {}\npage_cache_misses {}\n",
        state.metrics.requests_total.load(Ordering::Relaxed),
        state.metrics.page_cache_hits.load(Ordering::Relaxed),
        state.metrics.page_cache_misses.load(Ordering::Relaxed),
    )
}
