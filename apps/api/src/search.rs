use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{error::ApiError, AppState};

#[derive(Deserialize, ToSchema)]
pub struct SearchQuery {
    #[schema(value_type = String, example = "batman")]
    pub q: String,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<String>,
    #[schema(value_type = Option<String>, example = "cbz")]
    pub r#type: Option<String>,
    #[schema(value_type = Option<String>, example = "cbz")]
    pub kind: Option<String>,
    #[schema(value_type = Option<usize>, example = 20)]
    pub limit: Option<usize>,
}

#[derive(Serialize, ToSchema)]
pub struct SearchResponse {
    pub hits: serde_json::Value,
    pub estimated_total_hits: Option<u64>,
    pub processing_time_ms: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/search",
    tag = "books",
    params(
        ("q" = String, description = "Search query"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("type" = Option<String>, Query, description = "Filter by type (cbz, cbr, pdf)"),
        ("kind" = Option<String>, Query, description = "Filter by kind (alias for type)"),
        ("limit" = Option<usize>, Query, description = "Max results (max 100)"),
    ),
    responses(
        (status = 200, body = SearchResponse),
    )
)]
pub async fn search_books(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, ApiError> {
    if query.q.trim().is_empty() {
        return Err(ApiError::bad_request("q is required"));
    }

    let mut filters: Vec<String> = Vec::new();
    if let Some(library_id) = query.library_id.as_deref() {
        filters.push(format!("library_id = '{}'", library_id.replace('"', "")));
    }
    let kind_filter = query.r#type.as_deref().or(query.kind.as_deref());
    if let Some(kind) = kind_filter {
        filters.push(format!("kind = '{}'", kind.replace('"', "")));
    }

    let body = serde_json::json!({
        "q": query.q,
        "limit": query.limit.unwrap_or(20).clamp(1, 100),
        "filter": if filters.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(filters.join(" AND ")) }
    });

    let client = reqwest::Client::new();
    let url = format!("{}/indexes/books/search", state.meili_url.trim_end_matches('/'));
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", state.meili_master_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("meili request failed: {e}")))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_else(|_| "unknown meili error".to_string());
        if body.contains("index_not_found") {
            return Ok(Json(SearchResponse {
                hits: serde_json::json!([]),
                estimated_total_hits: Some(0),
                processing_time_ms: Some(0),
            }));
        }
        return Err(ApiError::internal(format!("meili error: {body}")));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("invalid meili response: {e}")))?;

    Ok(Json(SearchResponse {
        hits: payload.get("hits").cloned().unwrap_or_else(|| serde_json::json!([])),
        estimated_total_hits: payload.get("estimatedTotalHits").and_then(|v| v.as_u64()),
        processing_time_ms: payload.get("processingTimeMs").and_then(|v| v.as_u64()),
    }))
}
