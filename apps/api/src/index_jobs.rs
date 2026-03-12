use axum::{extract::State, response::sse::{Event, Sse}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::Stream;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct RebuildRequest {
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<bool>, example = false)]
    pub full: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct IndexJobResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub book_id: Option<Uuid>,
    pub r#type: String,
    pub status: String,
    #[schema(value_type = Option<String>)]
    pub started_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>)]
    pub finished_at: Option<DateTime<Utc>>,
    pub stats_json: Option<serde_json::Value>,
    pub error_opt: Option<String>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    pub progress_percent: Option<i32>,
    pub processed_files: Option<i32>,
    pub total_files: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct FolderItem {
    pub name: String,
    pub path: String,
    pub depth: usize,
    pub has_children: bool,
}

#[derive(Serialize, ToSchema)]
pub struct IndexJobDetailResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub book_id: Option<Uuid>,
    pub r#type: String,
    pub status: String,
    #[schema(value_type = Option<String>)]
    pub started_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>)]
    pub finished_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>)]
    pub phase2_started_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>)]
    pub generating_thumbnails_started_at: Option<DateTime<Utc>>,
    pub stats_json: Option<serde_json::Value>,
    pub error_opt: Option<String>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    pub current_file: Option<String>,
    pub progress_percent: Option<i32>,
    pub total_files: Option<i32>,
    pub processed_files: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct JobErrorResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub file_path: String,
    pub error_message: String,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct ProgressEvent {
    pub job_id: String,
    pub status: String,
    pub current_file: Option<String>,
    pub progress_percent: Option<i32>,
    pub processed_files: Option<i32>,
    pub total_files: Option<i32>,
    pub stats_json: Option<serde_json::Value>,
}

/// Enqueue a job to rebuild the search index for a library (or all libraries if no library_id specified)
#[utoipa::path(
    post,
    path = "/index/rebuild",
    tag = "indexing",
    request_body = Option<RebuildRequest>,
    responses(
        (status = 200, body = IndexJobResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn enqueue_rebuild(
    State(state): State<AppState>,
    payload: Option<Json<RebuildRequest>>,
) -> Result<Json<IndexJobResponse>, ApiError> {
    let library_id = payload.as_ref().and_then(|p| p.0.library_id);
    let is_full = payload.as_ref().and_then(|p| p.0.full).unwrap_or(false);
    let job_type = if is_full { "full_rebuild" } else { "rebuild" };
    let id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, 'pending')",
    )
    .bind(id)
    .bind(library_id)
    .bind(job_type)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, library_id, book_id, type, status, started_at, finished_at, stats_json, error_opt, created_at FROM index_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(map_row(row)))
}

/// List recent indexing jobs with their status
#[utoipa::path(
    get,
    path = "/index/status",
    tag = "indexing",
    responses(
        (status = 200, body = Vec<IndexJobResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_index_jobs(State(state): State<AppState>) -> Result<Json<Vec<IndexJobResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, library_id, book_id, type, status, started_at, finished_at, stats_json, error_opt, created_at, progress_percent, processed_files, total_files FROM index_jobs ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows.into_iter().map(map_row).collect()))
}

/// Cancel a pending or running indexing job
#[utoipa::path(
    post,
    path = "/index/cancel/{id}",
    tag = "indexing",
    params(
        ("id" = String, Path, description = "Job UUID"),
    ),
    responses(
        (status = 200, body = IndexJobResponse),
        (status = 404, description = "Job not found or already finished"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn cancel_job(
    State(state): State<AppState>,
    id: axum::extract::Path<Uuid>,
) -> Result<Json<IndexJobResponse>, ApiError> {
    let rows_affected = sqlx::query(
        "UPDATE index_jobs SET status = 'cancelled' WHERE id = $1 AND status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails')",
    )
    .bind(id.0)
    .execute(&state.pool)
    .await?;

    if rows_affected.rows_affected() == 0 {
        return Err(ApiError::not_found("job not found or already finished"));
    }

    let row = sqlx::query(
        "SELECT id, library_id, book_id, type, status, started_at, finished_at, stats_json, error_opt, created_at, progress_percent, processed_files, total_files FROM index_jobs WHERE id = $1",
    )
    .bind(id.0)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(map_row(row)))
}

fn get_libraries_root() -> String {
    std::env::var("LIBRARIES_ROOT_PATH").unwrap_or_else(|_| "/libraries".to_string())
}

/// List available folders in /libraries for library creation
/// Supports browsing subdirectories via optional path parameter
#[utoipa::path(
    get,
    path = "/folders",
    tag = "indexing",
    params(
        ("path" = Option<String>, Query, description = "Optional subdirectory path to browse (e.g., '/libraries/manga/action')"),
    ),
    responses(
        (status = 200, body = Vec<FolderItem>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_folders(
    State(_state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<FolderItem>>, ApiError> {
    let libraries_root = get_libraries_root();
    let base_path = std::path::Path::new(&libraries_root);
    
    // Determine which path to browse
    let target_path = if let Some(sub_path) = params.get("path") {
        // Validate the path to prevent directory traversal attacks
        if sub_path.contains("..") || sub_path.contains("~") {
            return Err(ApiError::bad_request("Invalid path"));
        }
        // Remove /libraries/ prefix if present since base_path is already /libraries
        let cleaned_path = sub_path.trim_start_matches("/libraries/").trim_start_matches('/');
        if cleaned_path.is_empty() {
            base_path.to_path_buf()
        } else {
            base_path.join(cleaned_path)
        }
    } else {
        base_path.to_path_buf()
    };
    
    // Ensure the path is within the libraries root
    let canonical_target = target_path.canonicalize().unwrap_or(target_path.clone());
    let canonical_base = base_path.canonicalize().unwrap_or(base_path.to_path_buf());
    
    if !canonical_target.starts_with(&canonical_base) {
        return Err(ApiError::bad_request("Path is outside libraries root"));
    }
    
    let mut folders = Vec::new();
    let depth = if params.contains_key("path") {
        canonical_target.strip_prefix(&canonical_base)
            .map(|p| p.components().count())
            .unwrap_or(0)
    } else {
        0
    };

    if let Ok(entries) = std::fs::read_dir(&canonical_target) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                
                // Check if this folder has children
                let has_children = if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                    sub_entries.flatten().any(|e| {
                        e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    })
                } else {
                    false
                };
                
                // Calculate the full path relative to libraries root
                let full_path = if let Ok(relative) = entry.path().strip_prefix(&canonical_base) {
                    format!("/libraries/{}", relative.to_string_lossy())
                } else {
                    format!("/libraries/{}", name)
                };
                
                folders.push(FolderItem {
                    name,
                    path: full_path,
                    depth,
                    has_children,
                });
            }
        }
    }

    folders.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(folders))
}

pub fn map_row(row: sqlx::postgres::PgRow) -> IndexJobResponse {
    IndexJobResponse {
        id: row.get("id"),
        library_id: row.get("library_id"),
        book_id: row.try_get("book_id").ok().flatten(),
        r#type: row.get("type"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        stats_json: row.get("stats_json"),
        error_opt: row.get("error_opt"),
        created_at: row.get("created_at"),
        progress_percent: row.try_get("progress_percent").ok(),
        processed_files: row.try_get("processed_files").ok(),
        total_files: row.try_get("total_files").ok(),
    }
}

fn map_row_detail(row: sqlx::postgres::PgRow) -> IndexJobDetailResponse {
    IndexJobDetailResponse {
        id: row.get("id"),
        library_id: row.get("library_id"),
        book_id: row.try_get("book_id").ok().flatten(),
        r#type: row.get("type"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        phase2_started_at: row.try_get("phase2_started_at").ok().flatten(),
        generating_thumbnails_started_at: row.try_get("generating_thumbnails_started_at").ok().flatten(),
        stats_json: row.get("stats_json"),
        error_opt: row.get("error_opt"),
        created_at: row.get("created_at"),
        current_file: row.get("current_file"),
        progress_percent: row.get("progress_percent"),
        total_files: row.get("total_files"),
        processed_files: row.get("processed_files"),
    }
}

/// List active indexing jobs (pending or running)
#[utoipa::path(
    get,
    path = "/index/jobs/active",
    tag = "indexing",
    responses(
        (status = 200, body = Vec<IndexJobResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_active_jobs(State(state): State<AppState>) -> Result<Json<Vec<IndexJobResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, library_id, book_id, type, status, started_at, finished_at, stats_json, error_opt, created_at, progress_percent, processed_files, total_files
         FROM index_jobs
         WHERE status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails')
         ORDER BY created_at ASC"
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows.into_iter().map(map_row).collect()))
}

/// Get detailed job information including progress
#[utoipa::path(
    get,
    path = "/index/jobs/{id}/details",
    tag = "indexing",
    params(
        ("id" = String, Path, description = "Job UUID"),
    ),
    responses(
        (status = 200, body = IndexJobDetailResponse),
        (status = 404, description = "Job not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_job_details(
    State(state): State<AppState>,
    id: axum::extract::Path<Uuid>,
) -> Result<Json<IndexJobDetailResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT id, library_id, book_id, type, status, started_at, finished_at, phase2_started_at, generating_thumbnails_started_at,
                stats_json, error_opt, created_at, current_file, progress_percent, total_files, processed_files
         FROM index_jobs WHERE id = $1"
    )
    .bind(id.0)
    .fetch_optional(&state.pool)
    .await?;

    match row {
        Some(row) => Ok(Json(map_row_detail(row))),
        None => Err(ApiError::not_found("job not found")),
    }
}

/// List errors for a specific job
#[utoipa::path(
    get,
    path = "/index/jobs/{id}/errors",
    tag = "indexing",
    params(
        ("id" = String, Path, description = "Job UUID"),
    ),
    responses(
        (status = 200, body = Vec<JobErrorResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_job_errors(
    State(state): State<AppState>,
    id: axum::extract::Path<Uuid>,
) -> Result<Json<Vec<JobErrorResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, file_path, error_message, created_at 
         FROM index_job_errors 
         WHERE job_id = $1 
         ORDER BY created_at ASC"
    )
    .bind(id.0)
    .fetch_all(&state.pool)
    .await?;

    let errors: Vec<JobErrorResponse> = rows
        .into_iter()
        .map(|row| JobErrorResponse {
            id: row.get("id"),
            file_path: row.get("file_path"),
            error_message: row.get("error_message"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(errors))
}

/// Stream job progress via SSE
#[utoipa::path(
    get,
    path = "/index/jobs/{id}/stream",
    tag = "indexing",
    params(
        ("id" = String, Path, description = "Job UUID"),
    ),
    responses(
        (status = 200, description = "SSE stream of progress events"),
        (status = 404, description = "Job not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn stream_job_progress(
    State(state): State<AppState>,
    id: axum::extract::Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Verify job exists
    let job_exists = sqlx::query("SELECT 1 FROM index_jobs WHERE id = $1")
        .bind(id.0)
        .fetch_optional(&state.pool)
        .await?;

    if job_exists.is_none() {
        return Err(ApiError::not_found("job not found"));
    }

    let job_id = id.0;
    let pool = state.pool.clone();

    let stream = async_stream::stream! {
        let mut last_status: Option<String> = None;
        let mut last_processed: Option<i32> = None;
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            interval.tick().await;

            let row = sqlx::query(
                "SELECT status, current_file, progress_percent, processed_files, total_files, stats_json
                 FROM index_jobs WHERE id = $1"
            )
            .bind(job_id)
            .fetch_one(&pool)
            .await;

            match row {
                Ok(row) => {
                    let status: String = row.get("status");
                    let processed_files: Option<i32> = row.get("processed_files");

                    // Send update if status changed or progress changed
                    let should_send = last_status.as_ref() != Some(&status)
                        || last_processed != processed_files;

                    if should_send {
                        last_status = Some(status.clone());
                        last_processed = processed_files;

                        let event = ProgressEvent {
                            job_id: job_id.to_string(),
                            status: status.clone(),
                            current_file: row.get("current_file"),
                            progress_percent: row.get("progress_percent"),
                            processed_files,
                            total_files: row.get("total_files"),
                            stats_json: row.get("stats_json"),
                        };

                        if let Ok(json) = serde_json::to_string(&event) {
                            yield Ok(Event::default().data(json));
                        }

                        // Stop streaming if job is finished
                        if status == "success" || status == "failed" || status == "cancelled" {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}
