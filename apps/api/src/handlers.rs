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

pub async fn api_home() -> impl axum::response::IntoResponse {
    let version = env!("CARGO_PKG_VERSION");
    axum::response::Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Stripstream Librarian API</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; display: flex; align-items: center; justify-content: center; }}
        .container {{ text-align: center; max-width: 480px; padding: 2rem; }}
        h1 {{ font-size: 2rem; margin-bottom: 0.5rem; color: #f8fafc; }}
        .version {{ color: #64748b; font-size: 0.875rem; margin-bottom: 2rem; }}
        .cards {{ display: flex; flex-direction: column; gap: 1rem; }}
        a {{ display: block; padding: 1.25rem 1.5rem; background: #1e293b; border: 1px solid #334155; border-radius: 0.75rem; text-decoration: none; color: #e2e8f0; transition: all 0.2s; }}
        a:hover {{ background: #334155; border-color: #3b82f6; transform: translateY(-2px); }}
        .label {{ font-size: 1.125rem; font-weight: 600; margin-bottom: 0.25rem; }}
        .desc {{ font-size: 0.8rem; color: #94a3b8; }}
        .badge {{ display: inline-block; font-size: 0.65rem; padding: 0.15rem 0.5rem; border-radius: 9999px; font-weight: 600; margin-left: 0.5rem; vertical-align: middle; }}
        .badge-blue {{ background: #1e40af; color: #93c5fd; }}
        .badge-amber {{ background: #78350f; color: #fcd34d; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Stripstream Librarian</h1>
        <p class="version">API v{version}</p>
        <div class="cards">
            <a href="/swagger-ui/?urls.primaryName=%2Fopenapi.json">
                <div class="label">Client API <span class="badge badge-blue">read</span></div>
                <div class="desc">Books, series, reading progress, search, thumbnails</div>
            </a>
            <a href="/swagger-ui/?urls.primaryName=%2Fadmin%2Fopenapi.json">
                <div class="label">Admin API <span class="badge badge-amber">admin</span></div>
                <div class="desc">All endpoints — metadata, jobs, settings, downloads, integrations</div>
            </a>
            <a href="/health">
                <div class="label">Health Check</div>
                <div class="desc">Service status</div>
            </a>
        </div>
    </div>
</body>
</html>"#))
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
