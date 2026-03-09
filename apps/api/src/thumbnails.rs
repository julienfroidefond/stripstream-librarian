use axum::{
    extract::State,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, index_jobs, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct ThumbnailsRebuildRequest {
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
}

/// POST /index/thumbnails/rebuild — create a job to generate thumbnails for books that don't have one.
#[utoipa::path(
    post,
    path = "/index/thumbnails/rebuild",
    tag = "indexing",
    request_body = Option<ThumbnailsRebuildRequest>,
    responses(
        (status = 200, body = index_jobs::IndexJobResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_thumbnails_rebuild(
    State(state): State<AppState>,
    payload: Option<Json<ThumbnailsRebuildRequest>>,
) -> Result<Json<index_jobs::IndexJobResponse>, ApiError> {
    let library_id = payload.as_ref().and_then(|p| p.0.library_id);
    let job_id = Uuid::new_v4();

    let row = sqlx::query(
        r#"INSERT INTO index_jobs (id, library_id, type, status)
           VALUES ($1, $2, 'thumbnail_rebuild', 'pending')
           RETURNING id, library_id, type, status, started_at, finished_at, stats_json, error_opt, created_at"#,
    )
    .bind(job_id)
    .bind(library_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(index_jobs::map_row(row)))
}

/// POST /index/thumbnails/regenerate — create a job to regenerate all thumbnails (clears then regenerates).
#[utoipa::path(
    post,
    path = "/index/thumbnails/regenerate",
    tag = "indexing",
    request_body = Option<ThumbnailsRebuildRequest>,
    responses(
        (status = 200, body = index_jobs::IndexJobResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_thumbnails_regenerate(
    State(state): State<AppState>,
    payload: Option<Json<ThumbnailsRebuildRequest>>,
) -> Result<Json<index_jobs::IndexJobResponse>, ApiError> {
    let library_id = payload.as_ref().and_then(|p| p.0.library_id);
    let job_id = Uuid::new_v4();

    let row = sqlx::query(
        r#"INSERT INTO index_jobs (id, library_id, type, status)
           VALUES ($1, $2, 'thumbnail_regenerate', 'pending')
           RETURNING id, library_id, type, status, started_at, finished_at, stats_json, error_opt, created_at"#,
    )
    .bind(job_id)
    .bind(library_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(index_jobs::map_row(row)))
}
