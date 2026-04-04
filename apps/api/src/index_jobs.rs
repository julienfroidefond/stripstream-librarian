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
    /// Deep rescan: clears directory mtimes to force re-walking all directories,
    /// discovering newly supported formats without deleting existing data.
    #[schema(value_type = Option<bool>, example = false)]
    pub rescan: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct IndexJobResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    pub library_name: Option<String>,
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
    let is_rescan = payload.as_ref().and_then(|p| p.0.rescan).unwrap_or(false);
    let job_type = if is_full { "full_rebuild" } else if is_rescan { "rescan" } else { "rebuild" };

    // When no library specified, create one job per library
    if library_id.is_none() {
        let library_ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM libraries ORDER BY name")
            .fetch_all(&state.pool)
            .await?;
        let mut last_id: Option<Uuid> = None;
        for lib_id in library_ids {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, 'pending')",
            )
            .bind(id)
            .bind(lib_id)
            .bind(job_type)
            .execute(&state.pool)
            .await?;
            last_id = Some(id);
        }
        let last_id = last_id.ok_or_else(|| ApiError::bad_request("No libraries found"))?;
        let row = sqlx::query(
            "SELECT j.id, j.library_id, l.name AS library_name, j.book_id, j.type, j.status, j.started_at, j.finished_at, j.stats_json, j.error_opt, j.created_at FROM index_jobs j LEFT JOIN libraries l ON l.id = j.library_id WHERE j.id = $1",
        )
        .bind(last_id)
        .fetch_one(&state.pool)
        .await?;
        return Ok(Json(map_row(row)));
    }

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
        "SELECT j.id, j.library_id, l.name AS library_name, j.book_id, j.type, j.status, j.started_at, j.finished_at, j.stats_json, j.error_opt, j.created_at FROM index_jobs j LEFT JOIN libraries l ON l.id = j.library_id WHERE j.id = $1",
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
        "SELECT j.id, j.library_id, l.name AS library_name, j.book_id, j.type, j.status, j.started_at, j.finished_at, j.stats_json, j.error_opt, j.created_at, j.progress_percent, j.processed_files, j.total_files FROM index_jobs j LEFT JOIN libraries l ON l.id = j.library_id ORDER BY j.created_at DESC LIMIT 100",
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
        "SELECT j.id, j.library_id, l.name AS library_name, j.book_id, j.type, j.status, j.started_at, j.finished_at, j.stats_json, j.error_opt, j.created_at, j.progress_percent, j.processed_files, j.total_files FROM index_jobs j LEFT JOIN libraries l ON l.id = j.library_id WHERE j.id = $1",
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
    
    // Ensure the path is within the libraries root (avoid canonicalize — burns fd on Docker mounts)
    let canonical_target = target_path.clone();
    let canonical_base = base_path.to_path_buf();
    
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

    let entries = std::fs::read_dir(&canonical_target)
        .map_err(|e| ApiError::internal(format!("cannot read directory {}: {}", canonical_target.display(), e)))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("[FOLDERS] entry error in {}: {}", canonical_target.display(), e);
                continue;
            }
        };
        let is_dir = match entry.file_type() {
            Ok(ft) => ft.is_dir(),
            Err(e) => {
                tracing::warn!("[FOLDERS] cannot stat {}: {}", entry.path().display(), e);
                continue;
            }
        };
        if is_dir {
                let name = entry.file_name().to_string_lossy().to_string();

                // Check if this folder has children (best-effort, default to true on error)
                let has_children = std::fs::read_dir(entry.path())
                    .map(|sub| sub.flatten().any(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)))
                    .unwrap_or(true);
                
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

    folders.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(folders))
}

pub fn map_row(row: sqlx::postgres::PgRow) -> IndexJobResponse {
    IndexJobResponse {
        id: row.get("id"),
        library_id: row.get("library_id"),
        library_name: row.try_get("library_name").ok().flatten(),
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
        "SELECT j.id, j.library_id, l.name AS library_name, j.book_id, j.type, j.status, j.started_at, j.finished_at, j.stats_json, j.error_opt, j.created_at, j.progress_percent, j.processed_files, j.total_files
         FROM index_jobs j LEFT JOIN libraries l ON l.id = j.library_id
         WHERE j.status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails')
         ORDER BY j.created_at ASC"
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

/// List books that were indexed (created or updated) during a scan/rebuild job.
/// Uses the job's time window (started_at → finished_at) and library_id to find
/// books whose updated_at falls within that range.
#[utoipa::path(
    get,
    path = "/index/jobs/{id}/indexed-books",
    tag = "indexing",
    params(
        ("id" = String, Path, description = "Job UUID"),
    ),
    responses(
        (status = 200, description = "List of indexed books"),
        (status = 404, description = "Job not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_indexed_books(
    State(state): State<AppState>,
    id: axum::extract::Path<Uuid>,
) -> Result<Json<Vec<IndexedBookDto>>, ApiError> {
    let job = sqlx::query(
        "SELECT library_id, started_at, finished_at FROM index_jobs WHERE id = $1",
    )
    .bind(id.0)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("job not found"))?;

    let library_id: Option<Uuid> = job.get("library_id");
    let started_at: Option<DateTime<Utc>> = job.get("started_at");
    let finished_at: Option<DateTime<Utc>> = job.get("finished_at");

    let (started_at, finished_at) = match (started_at, finished_at) {
        (Some(s), Some(f)) => (s, f),
        _ => return Ok(Json(vec![])),
    };

    let rows = sqlx::query(
        "SELECT b.id, b.title, b.volume, s.name AS series_name, b.kind, \
                bf.abs_path, bf.format \
         FROM books b \
         LEFT JOIN series s ON s.id = b.series_id \
         LEFT JOIN LATERAL (SELECT abs_path, format FROM book_files WHERE book_id = b.id ORDER BY updated_at DESC LIMIT 1) bf ON TRUE \
         WHERE ($1::uuid IS NULL OR b.library_id = $1) \
           AND b.updated_at >= $2 AND b.updated_at <= $3 \
         ORDER BY s.name, b.volume, b.title \
         LIMIT 500",
    )
    .bind(library_id)
    .bind(started_at)
    .bind(finished_at)
    .fetch_all(&state.pool)
    .await?;

    let books: Vec<IndexedBookDto> = rows
        .iter()
        .map(|row| IndexedBookDto {
            id: row.get("id"),
            title: row.get("title"),
            volume: row.get("volume"),
            series_name: row.get("series_name"),
            kind: row.get("kind"),
            file_path: row.get("abs_path"),
            format: row.get("format"),
        })
        .collect();

    Ok(Json(books))
}

#[derive(Serialize, ToSchema)]
pub struct IndexedBookDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub title: String,
    pub volume: Option<i32>,
    pub series_name: Option<String>,
    pub kind: String,
    pub file_path: Option<String>,
    pub format: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct JobEventDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub job_id: Uuid,
    pub event_type: String,
    pub level: String,
    pub entity_type: Option<String>,
    #[schema(value_type = Option<String>)]
    pub entity_id: Option<Uuid>,
    pub entity_name: Option<String>,
    pub message: Option<String>,
    pub detail: Option<serde_json::Value>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct JobEventsQuery {
    pub level: Option<String>,
    pub event_type: Option<String>,
    pub limit: Option<i64>,
}

/// List events for a specific job
#[utoipa::path(
    get,
    path = "/index/jobs/{id}/events",
    tag = "indexing",
    params(
        ("id" = String, Path, description = "Job UUID"),
        ("level" = Option<String>, Query, description = "Filter by level: info, warning, error"),
        ("event_type" = Option<String>, Query, description = "Filter by event_type"),
        ("limit" = Option<i64>, Query, description = "Max results (default 500)"),
    ),
    responses(
        (status = 200, body = Vec<JobEventDto>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_job_events(
    State(state): State<AppState>,
    id: axum::extract::Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<JobEventsQuery>,
) -> Result<Json<Vec<JobEventDto>>, ApiError> {
    let limit = query.limit.unwrap_or(500).min(5000);

    let rows = sqlx::query(
        "SELECT id, job_id, event_type, level, entity_type, entity_id, entity_name, message, detail, created_at \
         FROM index_job_events \
         WHERE job_id = $1 \
           AND ($2::text IS NULL OR level = $2) \
           AND ($3::text IS NULL OR event_type = $3) \
         ORDER BY created_at ASC \
         LIMIT $4",
    )
    .bind(id.0)
    .bind(&query.level)
    .bind(&query.event_type)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    let events: Vec<JobEventDto> = rows
        .into_iter()
        .map(|row| JobEventDto {
            id: row.get("id"),
            job_id: row.get("job_id"),
            event_type: row.get("event_type"),
            level: row.get("level"),
            entity_type: row.get("entity_type"),
            entity_id: row.get("entity_id"),
            entity_name: row.get("entity_name"),
            message: row.get("message"),
            detail: row.get("detail"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(events))
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

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    async fn create_test_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(name)
            .bind(format!("/libraries/{name}"))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    /// The list query used in list_index_jobs / get_active_jobs / cancel_job.
    const LIST_JOBS_SQL: &str =
        "SELECT j.id, j.library_id, l.name AS library_name, j.book_id, j.type, j.status, j.started_at, j.finished_at, j.stats_json, j.error_opt, j.created_at, j.progress_percent, j.processed_files, j.total_files FROM index_jobs j LEFT JOIN libraries l ON l.id = j.library_id WHERE j.id = $1";

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn job_with_library_has_library_name(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "My Comics").await;
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
        )
        .bind(job_id)
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query(LIST_JOBS_SQL)
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        let resp = super::map_row(row);
        assert_eq!(resp.id, job_id);
        assert_eq!(resp.library_id, Some(lib_id));
        assert_eq!(resp.library_name.as_deref(), Some("My Comics"));
        assert_eq!(resp.r#type, "scan");
        assert_eq!(resp.status, "pending");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn job_without_library_has_null_library_name(pool: sqlx::PgPool) {
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, NULL, 'scan', 'pending')",
        )
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();

        let row = sqlx::query(LIST_JOBS_SQL)
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        let resp = super::map_row(row);
        assert_eq!(resp.id, job_id);
        assert!(resp.library_id.is_none());
        assert!(resp.library_name.is_none(), "library_name should be null when no library_id");
    }

    // ── Helper to create a job and insert events ──────────────────────

    async fn create_test_job(pool: &sqlx::PgPool) -> Uuid {
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, NULL, 'scan', 'running')",
        )
        .bind(job_id)
        .execute(pool)
        .await
        .unwrap();
        job_id
    }

    async fn insert_event(
        pool: &sqlx::PgPool,
        job_id: Uuid,
        event_type: &str,
        level: &str,
        entity_type: Option<&str>,
        entity_name: Option<&str>,
        message: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name, message) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(job_id)
        .bind(event_type)
        .bind(level)
        .bind(entity_type)
        .bind(entity_name)
        .bind(message)
        .execute(pool)
        .await
        .unwrap();
    }

    const EVENTS_SQL: &str =
        "SELECT id, job_id, event_type, level, entity_type, entity_id, entity_name, message, detail, created_at \
         FROM index_job_events \
         WHERE job_id = $1 \
           AND ($2::text IS NULL OR level = $2) \
           AND ($3::text IS NULL OR event_type = $3) \
         ORDER BY created_at ASC \
         LIMIT $4";

    fn map_event_rows(rows: Vec<sqlx::postgres::PgRow>) -> Vec<super::JobEventDto> {
        use sqlx::Row;
        rows.into_iter()
            .map(|row| super::JobEventDto {
                id: row.get("id"),
                job_id: row.get("job_id"),
                event_type: row.get("event_type"),
                level: row.get("level"),
                entity_type: row.get("entity_type"),
                entity_id: row.get("entity_id"),
                entity_name: row.get("entity_name"),
                message: row.get("message"),
                detail: row.get("detail"),
                created_at: row.get("created_at"),
            })
            .collect()
    }

    // ── get_job_events tests ──────────────────────────────────────────

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_job_events_returns_all_events_ordered(pool: sqlx::PgPool) {
        let job_id = create_test_job(&pool).await;

        insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("Book A"), None).await;
        insert_event(&pool, job_id, "book_updated", "warning", Some("book"), Some("Book B"), Some("cover missing")).await;
        insert_event(&pool, job_id, "parse_error", "error", Some("book"), Some("Book C"), Some("corrupt archive")).await;

        let rows = sqlx::query(EVENTS_SQL)
            .bind(job_id)
            .bind(None::<String>) // no level filter
            .bind(None::<String>) // no event_type filter
            .bind(500_i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        let events = map_event_rows(rows);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, "book_added");
        assert_eq!(events[1].event_type, "book_updated");
        assert_eq!(events[2].event_type, "parse_error");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_job_events_filter_by_level(pool: sqlx::PgPool) {
        let job_id = create_test_job(&pool).await;

        insert_event(&pool, job_id, "book_added", "info", None, None, None).await;
        insert_event(&pool, job_id, "book_updated", "warning", None, None, None).await;
        insert_event(&pool, job_id, "parse_error", "error", None, None, Some("bad file")).await;
        insert_event(&pool, job_id, "thumbnail_error", "error", None, None, Some("bad image")).await;

        let rows = sqlx::query(EVENTS_SQL)
            .bind(job_id)
            .bind(Some("error"))
            .bind(None::<String>)
            .bind(500_i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        let events = map_event_rows(rows);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.level == "error"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_job_events_filter_by_event_type(pool: sqlx::PgPool) {
        let job_id = create_test_job(&pool).await;

        insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("A"), None).await;
        insert_event(&pool, job_id, "book_added", "info", Some("book"), Some("B"), None).await;
        insert_event(&pool, job_id, "book_updated", "info", Some("book"), Some("C"), None).await;

        let rows = sqlx::query(EVENTS_SQL)
            .bind(job_id)
            .bind(None::<String>)
            .bind(Some("book_added"))
            .bind(500_i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        let events = map_event_rows(rows);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.event_type == "book_added"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_job_events_with_limit(pool: sqlx::PgPool) {
        let job_id = create_test_job(&pool).await;

        for i in 0..10 {
            insert_event(&pool, job_id, "book_added", "info", None, Some(&format!("Book {i}")), None).await;
        }

        let rows = sqlx::query(EVENTS_SQL)
            .bind(job_id)
            .bind(None::<String>)
            .bind(None::<String>)
            .bind(3_i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        let events = map_event_rows(rows);
        assert_eq!(events.len(), 3);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_job_events_empty_for_job_with_no_events(pool: sqlx::PgPool) {
        let job_id = create_test_job(&pool).await;

        let rows = sqlx::query(EVENTS_SQL)
            .bind(job_id)
            .bind(None::<String>)
            .bind(None::<String>)
            .bind(500_i64)
            .fetch_all(&pool)
            .await
            .unwrap();

        let events = map_event_rows(rows);
        assert!(events.is_empty());
    }
}
