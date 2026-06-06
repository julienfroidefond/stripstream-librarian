use axum::extract::Extension;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::helpers::{create_series_with_metadata, CreateSeriesParams};
use crate::{auth::AuthUser, error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct CreateSeriesRequest {
    pub library_id: String,
    pub name: String,
    /// Provider name (e.g., "senscritique", "bedetheque")
    pub provider: Option<String>,
    /// External ID from provider search result
    pub external_id: Option<String>,
    /// External URL
    pub external_url: Option<String>,
    /// Confidence score from search
    pub confidence: Option<f32>,
    /// Total volumes from provider
    pub total_volumes: Option<i32>,
    /// Provider metadata JSON
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Serialize, ToSchema)]
pub struct CreateSeriesResponse {
    #[schema(value_type = String)]
    pub series_id: Uuid,
    pub name: String,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub metadata_linked: bool,
    pub metadata_synced: bool,
}

/// Create a new series in a library, optionally linking and syncing metadata in one step.
#[utoipa::path(
    post,
    path = "/series/create",
    tag = "series",
    request_body = CreateSeriesRequest,
    responses(
        (status = 200, body = CreateSeriesResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn create_series(
    State(state): State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Json(body): Json<CreateSeriesRequest>,
) -> Result<Json<CreateSeriesResponse>, ApiError> {
    let library_id: Uuid = body
        .library_id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid library_id"))?;

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("name is required"));
    }

    let result = create_series_with_metadata(
        &state,
        CreateSeriesParams {
            library_id,
            name: name.clone(),
            provider: body.provider,
            external_id: body.external_id,
            external_url: body.external_url,
            confidence: body.confidence,
            total_volumes: body.total_volumes,
            metadata_json: body.metadata_json,
        },
    )
    .await?;

    Ok(Json(CreateSeriesResponse {
        series_id: result.series_id,
        name,
        library_id,
        metadata_linked: result.metadata_link_id.is_some(),
        metadata_synced: result.metadata_link_id.is_some(),
    }))
}
