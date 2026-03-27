use axum::{extract::State, http::StatusCode, Json};
use serde_json;

use crate::AppState;

pub async fn health() -> &'static str {
    "ok"
}

pub async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "indexer": env!("CARGO_PKG_VERSION"),
    }))
}

pub async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(serde_json::json!({"status": "ready"})))
}
