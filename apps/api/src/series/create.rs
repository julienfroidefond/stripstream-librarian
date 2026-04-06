use axum::extract::Extension;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{auth::AuthUser, error::ApiError, state::AppState};
use super::helpers::get_or_create_series;

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
///
/// If `provider` and `external_id` are provided, creates an approved metadata link
/// and syncs series + book metadata from the provider.
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

    // 1. Create or find the series
    let series_id = get_or_create_series(&state.pool, library_id, &name).await?;

    // 2. Create the physical directory on disk
    let root_path: String = sqlx::query_scalar("SELECT root_path FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;
    let physical_root = stripstream_core::paths::remap_libraries_path(&root_path);
    let series_dir = std::path::Path::new(&physical_root).join(&name);
    if !series_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&series_dir) {
            tracing::warn!("[SERIES] Failed to create directory {}: {}", series_dir.display(), e);
        } else {
            tracing::info!("[SERIES] Created directory: {}", series_dir.display());
        }
    }

    let mut metadata_linked = false;
    let mut metadata_synced = false;

    // 2. If metadata info provided, create approved link + sync
    if let (Some(ref provider), Some(ref external_id)) = (&body.provider, &body.external_id) {
        let metadata_json = body.metadata_json.clone().unwrap_or(serde_json::json!({}));

        // Create approved metadata link (reuses same SQL pattern as metadata/handlers.rs)
        let link_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO external_metadata_links
                (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
            VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8)
            ON CONFLICT (series_id, provider)
            DO UPDATE SET
                external_id = EXCLUDED.external_id,
                external_url = EXCLUDED.external_url,
                status = 'approved',
                confidence = EXCLUDED.confidence,
                metadata_json = EXCLUDED.metadata_json,
                total_volumes_external = EXCLUDED.total_volumes_external,
                matched_at = NOW(),
                approved_at = NOW(),
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(library_id)
        .bind(series_id)
        .bind(provider)
        .bind(external_id)
        .bind(&body.external_url)
        .bind(body.confidence)
        .bind(&metadata_json)
        .bind(body.total_volumes)
        .fetch_one(&state.pool)
        .await?;

        metadata_linked = true;

        // Sync series metadata (reuses metadata::sync::sync_series_metadata)
        let _ = crate::metadata::sync_series_metadata(
            &state, library_id, &name, &metadata_json, body.total_volumes,
        )
        .await;

        // Sync book metadata (reuses metadata::sync::sync_books_metadata)
        let _ = crate::metadata::sync_books_metadata(
            &state, link_id, library_id, &name, provider, external_id,
        )
        .await;

        metadata_synced = true;
    }

    Ok(Json(CreateSeriesResponse {
        series_id,
        name,
        library_id,
        metadata_linked,
        metadata_synced,
    }))
}
