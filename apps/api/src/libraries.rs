use std::path::{Path, PathBuf};

use axum::{extract::{Path as AxumPath, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};

#[derive(Serialize, ToSchema)]
pub struct LibraryResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub root_path: String,
    pub enabled: bool,
    pub book_count: i64,
    pub monitor_enabled: bool,
    pub scan_mode: String,
    #[schema(value_type = Option<String>)]
    pub next_scan_at: Option<chrono::DateTime<chrono::Utc>>,
    pub watcher_enabled: bool,
    pub metadata_provider: Option<String>,
    pub fallback_metadata_provider: Option<String>,
    pub metadata_refresh_mode: String,
    #[schema(value_type = Option<String>)]
    pub next_metadata_refresh_at: Option<chrono::DateTime<chrono::Utc>>,
    pub series_count: i64,
    /// First book IDs from up to 5 distinct series (for thumbnail fan display)
    #[schema(value_type = Vec<String>)]
    pub thumbnail_book_ids: Vec<Uuid>,
    pub reading_status_provider: Option<String>,
    pub reading_status_push_mode: String,
    #[schema(value_type = Option<String>)]
    pub next_reading_status_push_at: Option<chrono::DateTime<chrono::Utc>>,
    pub download_detection_mode: String,
    #[schema(value_type = Option<String>)]
    pub next_download_detection_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateLibraryRequest {
    #[schema(value_type = String, example = "Comics")]
    pub name: String,
    #[schema(value_type = String, example = "/data/comics")]
    pub root_path: String,
}

/// List all libraries with their book counts
#[utoipa::path(
    get,
    path = "/libraries",
    tag = "libraries",
    responses(
        (status = 200, body = Vec<LibraryResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_libraries(State(state): State<AppState>) -> Result<Json<Vec<LibraryResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT l.id, l.name, l.root_path, l.enabled, l.monitor_enabled, l.scan_mode, l.next_scan_at, l.watcher_enabled, l.metadata_provider, l.fallback_metadata_provider, l.metadata_refresh_mode, l.next_metadata_refresh_at, l.reading_status_provider, l.reading_status_push_mode, l.next_reading_status_push_at, l.download_detection_mode, l.next_download_detection_at,
                (SELECT COUNT(*) FROM books b WHERE b.library_id = l.id) as book_count,
                (SELECT COUNT(DISTINCT COALESCE(NULLIF(b.series, ''), 'unclassified')) FROM books b WHERE b.library_id = l.id) as series_count,
                COALESCE((
                    SELECT ARRAY_AGG(first_id ORDER BY series_name)
                    FROM (
                        SELECT DISTINCT ON (COALESCE(NULLIF(b.series, ''), 'unclassified'))
                            COALESCE(NULLIF(b.series, ''), 'unclassified') as series_name,
                            b.id as first_id
                        FROM books b
                        WHERE b.library_id = l.id
                        ORDER BY COALESCE(NULLIF(b.series, ''), 'unclassified'),
                            b.volume NULLS LAST, b.title ASC
                        LIMIT 5
                    ) sub
                ), ARRAY[]::uuid[]) as thumbnail_book_ids
         FROM libraries l ORDER BY l.created_at DESC"
    )
        .fetch_all(&state.pool)
        .await?;

    let items = rows
        .into_iter()
        .map(|row| LibraryResponse {
            id: row.get("id"),
            name: row.get("name"),
            root_path: row.get("root_path"),
            enabled: row.get("enabled"),
            book_count: row.get("book_count"),
            series_count: row.get("series_count"),
            monitor_enabled: row.get("monitor_enabled"),
            scan_mode: row.get("scan_mode"),
            next_scan_at: row.get("next_scan_at"),
            watcher_enabled: row.get("watcher_enabled"),
            metadata_provider: row.get("metadata_provider"),
            fallback_metadata_provider: row.get("fallback_metadata_provider"),
            metadata_refresh_mode: row.get("metadata_refresh_mode"),
            next_metadata_refresh_at: row.get("next_metadata_refresh_at"),
            thumbnail_book_ids: row.get("thumbnail_book_ids"),
            reading_status_provider: row.get("reading_status_provider"),
            reading_status_push_mode: row.get("reading_status_push_mode"),
            next_reading_status_push_at: row.get("next_reading_status_push_at"),
            download_detection_mode: row.get("download_detection_mode"),
            next_download_detection_at: row.get("next_download_detection_at"),
        })
        .collect();

    Ok(Json(items))
}

/// Create a new library from an absolute path
#[utoipa::path(
    post,
    path = "/libraries",
    tag = "libraries",
    request_body = CreateLibraryRequest,
    responses(
        (status = 200, body = LibraryResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn create_library(
    State(state): State<AppState>,
    Json(input): Json<CreateLibraryRequest>,
) -> Result<Json<LibraryResponse>, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::bad_request("name is required"));
    }

    let canonical = canonicalize_library_root(&input.root_path)?;
    let id = Uuid::new_v4();
    let root_path = canonical.to_string_lossy().to_string();

    sqlx::query(
        "INSERT INTO libraries (id, name, root_path, enabled) VALUES ($1, $2, $3, TRUE)",
    )
    .bind(id)
    .bind(input.name.trim())
    .bind(&root_path)
    .execute(&state.pool)
    .await?;

    Ok(Json(LibraryResponse {
        id,
        name: input.name.trim().to_string(),
        root_path,
        enabled: true,
        book_count: 0,
        series_count: 0,
        monitor_enabled: false,
        scan_mode: "manual".to_string(),
        next_scan_at: None,
        watcher_enabled: false,
        metadata_provider: None,
        fallback_metadata_provider: None,
        metadata_refresh_mode: "manual".to_string(),
        next_metadata_refresh_at: None,
        thumbnail_book_ids: vec![],
        reading_status_provider: None,
        reading_status_push_mode: "manual".to_string(),
        next_reading_status_push_at: None,
        download_detection_mode: "manual".to_string(),
        next_download_detection_at: None,
    }))
}

/// Delete a library by ID
#[utoipa::path(
    delete,
    path = "/libraries/{id}",
    tag = "libraries",
    params(
        ("id" = String, Path, description = "Library UUID"),
    ),
    responses(
        (status = 200, description = "Library deleted"),
        (status = 404, description = "Library not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_library(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    let result = sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("library not found"));
    }

    Ok(Json(crate::responses::DeletedResponse::new(id)))
}

fn canonicalize_library_root(root_path: &str) -> Result<PathBuf, ApiError> {
    let path = Path::new(root_path);
    if !path.is_absolute() {
        return Err(ApiError::bad_request("root_path must be absolute"));
    }

    // Avoid fs::canonicalize — it opens extra file descriptors to resolve symlinks
    // and can fail on Docker volume mounts (ro, cached) when fd limits are low.
    if !path.exists() {
        return Err(ApiError::bad_request(format!(
            "root_path does not exist: {}",
            root_path
        )));
    }
    if !path.is_dir() {
        return Err(ApiError::bad_request("root_path must point to a directory"));
    }

    Ok(path.to_path_buf())
}

use crate::index_jobs::{IndexJobResponse, RebuildRequest};

/// Trigger a scan/indexing job for a specific library
#[utoipa::path(
    post,
    path = "/libraries/{id}/scan",
    tag = "libraries",
    params(
        ("id" = String, Path, description = "Library UUID"),
    ),
    request_body = Option<RebuildRequest>,
    responses(
        (status = 200, body = IndexJobResponse),
        (status = 404, description = "Library not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn scan_library(
    State(state): State<AppState>,
    AxumPath(library_id): AxumPath<Uuid>,
    payload: Option<Json<RebuildRequest>>,
) -> Result<Json<IndexJobResponse>, ApiError> {
    // Verify library exists
    let library_exists = sqlx::query("SELECT 1 FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?;

    if library_exists.is_none() {
        return Err(ApiError::not_found("library not found"));
    }

    let is_full = payload.as_ref().and_then(|p| p.full).unwrap_or(false);
    let is_rescan = payload.as_ref().and_then(|p| p.rescan).unwrap_or(false);
    let job_type = if is_full { "full_rebuild" } else if is_rescan { "rescan" } else { "rebuild" };

    // Create indexing job for this library
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, 'pending')",
    )
    .bind(job_id)
    .bind(library_id)
    .bind(job_type)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, library_id, type, status, started_at, finished_at, stats_json, error_opt, created_at FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(crate::index_jobs::map_row(row)))
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMonitoringRequest {
    pub monitor_enabled: bool,
    #[schema(value_type = String, example = "hourly")]
    pub scan_mode: String, // 'manual', 'hourly', 'daily', 'weekly'
    pub watcher_enabled: Option<bool>,
    #[schema(value_type = Option<String>, example = "daily")]
    pub metadata_refresh_mode: Option<String>, // 'manual', 'hourly', 'daily', 'weekly'
    #[schema(value_type = Option<String>, example = "daily")]
    pub download_detection_mode: Option<String>, // 'manual', 'hourly', 'daily', 'weekly'
}

/// Update monitoring settings for a library
#[utoipa::path(
    patch,
    path = "/libraries/{id}/monitoring",
    tag = "libraries",
    params(
        ("id" = String, Path, description = "Library UUID"),
    ),
    request_body = UpdateMonitoringRequest,
    responses(
        (status = 200, body = LibraryResponse),
        (status = 404, description = "Library not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_monitoring(
    State(state): State<AppState>,
    AxumPath(library_id): AxumPath<Uuid>,
    Json(input): Json<UpdateMonitoringRequest>,
) -> Result<Json<LibraryResponse>, ApiError> {
    use stripstream_core::schedule::{validate_schedule_mode, mode_to_interval_minutes};

    // Validate scan_mode
    validate_schedule_mode(&input.scan_mode)
        .map_err(|e| ApiError::bad_request(format!("scan_mode {e}")))?;

    // Validate metadata_refresh_mode
    let metadata_refresh_mode = input.metadata_refresh_mode.as_deref().unwrap_or("manual");
    validate_schedule_mode(metadata_refresh_mode)
        .map_err(|e| ApiError::bad_request(format!("metadata_refresh_mode {e}")))?;

    // Validate download_detection_mode
    let download_detection_mode = input.download_detection_mode.as_deref().unwrap_or("manual");
    validate_schedule_mode(download_detection_mode)
        .map_err(|e| ApiError::bad_request(format!("download_detection_mode {e}")))?;

    // Calculate next_scan_at if monitoring is enabled
    let next_scan_at = if input.monitor_enabled {
        Some(chrono::Utc::now() + chrono::Duration::minutes(mode_to_interval_minutes(&input.scan_mode)))
    } else {
        None
    };

    // Calculate next_metadata_refresh_at
    let next_metadata_refresh_at = if metadata_refresh_mode != "manual" {
        Some(chrono::Utc::now() + chrono::Duration::minutes(mode_to_interval_minutes(metadata_refresh_mode)))
    } else {
        None
    };

    // Calculate next_download_detection_at
    let next_download_detection_at = if download_detection_mode != "manual" {
        Some(chrono::Utc::now() + chrono::Duration::minutes(mode_to_interval_minutes(download_detection_mode)))
    } else {
        None
    };

    let watcher_enabled = input.watcher_enabled.unwrap_or(false);

    let result = sqlx::query(
        "UPDATE libraries SET monitor_enabled = $2, scan_mode = $3, next_scan_at = $4, watcher_enabled = $5, metadata_refresh_mode = $6, next_metadata_refresh_at = $7, download_detection_mode = $8, next_download_detection_at = $9 WHERE id = $1 RETURNING id, name, root_path, enabled, monitor_enabled, scan_mode, next_scan_at, watcher_enabled, metadata_provider, fallback_metadata_provider, metadata_refresh_mode, next_metadata_refresh_at, reading_status_provider, reading_status_push_mode, next_reading_status_push_at, download_detection_mode, next_download_detection_at"
    )
    .bind(library_id)
    .bind(input.monitor_enabled)
    .bind(input.scan_mode)
    .bind(next_scan_at)
    .bind(watcher_enabled)
    .bind(metadata_refresh_mode)
    .bind(next_metadata_refresh_at)
    .bind(download_detection_mode)
    .bind(next_download_detection_at)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = result else {
        return Err(ApiError::not_found("library not found"));
    };

    let book_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE library_id = $1")
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;

    let series_count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT COALESCE(NULLIF(series, ''), 'unclassified')) FROM books WHERE library_id = $1")
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;

    let thumbnail_book_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT b.id FROM books b
         WHERE b.library_id = $1
         ORDER BY COALESCE(NULLIF(b.series, ''), 'unclassified'), b.volume NULLS LAST, b.title ASC
         LIMIT 5"
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(LibraryResponse {
        id: row.get("id"),
        name: row.get("name"),
        root_path: row.get("root_path"),
        enabled: row.get("enabled"),
        book_count,
        series_count,
        monitor_enabled: row.get("monitor_enabled"),
        scan_mode: row.get("scan_mode"),
        next_scan_at: row.get("next_scan_at"),
        watcher_enabled: row.get("watcher_enabled"),
        metadata_provider: row.get("metadata_provider"),
        fallback_metadata_provider: row.get("fallback_metadata_provider"),
        metadata_refresh_mode: row.get("metadata_refresh_mode"),
        next_metadata_refresh_at: row.get("next_metadata_refresh_at"),
        thumbnail_book_ids,
        reading_status_provider: row.get("reading_status_provider"),
        reading_status_push_mode: row.get("reading_status_push_mode"),
        next_reading_status_push_at: row.get("next_reading_status_push_at"),
        download_detection_mode: row.get("download_detection_mode"),
        next_download_detection_at: row.get("next_download_detection_at"),
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMetadataProviderRequest {
    pub metadata_provider: Option<String>,
    pub fallback_metadata_provider: Option<String>,
}

/// Update the metadata provider for a library
#[utoipa::path(
    patch,
    path = "/libraries/{id}/metadata-provider",
    tag = "libraries",
    params(
        ("id" = String, Path, description = "Library UUID"),
    ),
    request_body = UpdateMetadataProviderRequest,
    responses(
        (status = 200, body = LibraryResponse),
        (status = 404, description = "Library not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_metadata_provider(
    State(state): State<AppState>,
    AxumPath(library_id): AxumPath<Uuid>,
    Json(input): Json<UpdateMetadataProviderRequest>,
) -> Result<Json<LibraryResponse>, ApiError> {
    let provider = input.metadata_provider.as_deref().filter(|s| !s.is_empty());
    let fallback = input.fallback_metadata_provider.as_deref().filter(|s| !s.is_empty());

    let result = sqlx::query(
        "UPDATE libraries SET metadata_provider = $2, fallback_metadata_provider = $3 WHERE id = $1 RETURNING id, name, root_path, enabled, monitor_enabled, scan_mode, next_scan_at, watcher_enabled, metadata_provider, fallback_metadata_provider, metadata_refresh_mode, next_metadata_refresh_at, reading_status_provider, reading_status_push_mode, next_reading_status_push_at, download_detection_mode, next_download_detection_at"
    )
    .bind(library_id)
    .bind(provider)
    .bind(fallback)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = result else {
        return Err(ApiError::not_found("library not found"));
    };

    let book_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE library_id = $1")
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;

    let series_count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT COALESCE(NULLIF(series, ''), 'unclassified')) FROM books WHERE library_id = $1")
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;

    let thumbnail_book_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT b.id FROM books b
         WHERE b.library_id = $1
         ORDER BY COALESCE(NULLIF(b.series, ''), 'unclassified'), b.volume NULLS LAST, b.title ASC
         LIMIT 5"
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(LibraryResponse {
        id: row.get("id"),
        name: row.get("name"),
        root_path: row.get("root_path"),
        enabled: row.get("enabled"),
        book_count,
        series_count,
        monitor_enabled: row.get("monitor_enabled"),
        scan_mode: row.get("scan_mode"),
        next_scan_at: row.get("next_scan_at"),
        watcher_enabled: row.get("watcher_enabled"),
        metadata_provider: row.get("metadata_provider"),
        fallback_metadata_provider: row.get("fallback_metadata_provider"),
        metadata_refresh_mode: row.get("metadata_refresh_mode"),
        next_metadata_refresh_at: row.get("next_metadata_refresh_at"),
        thumbnail_book_ids,
        reading_status_provider: row.get("reading_status_provider"),
        reading_status_push_mode: row.get("reading_status_push_mode"),
        next_reading_status_push_at: row.get("next_reading_status_push_at"),
        download_detection_mode: row.get("download_detection_mode"),
        next_download_detection_at: row.get("next_download_detection_at"),
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateReadingStatusProviderRequest {
    pub reading_status_provider: Option<String>,
    pub reading_status_push_mode: Option<String>,
}

/// Update the reading status provider for a library
#[utoipa::path(
    patch,
    path = "/libraries/{id}/reading-status-provider",
    tag = "libraries",
    params(("id" = String, Path, description = "Library UUID")),
    request_body = UpdateReadingStatusProviderRequest,
    responses(
        (status = 200, description = "Updated"),
        (status = 404, description = "Library not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_reading_status_provider(
    State(state): State<AppState>,
    AxumPath(library_id): AxumPath<Uuid>,
    Json(input): Json<UpdateReadingStatusProviderRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let provider = input.reading_status_provider.as_deref().filter(|s| !s.is_empty());

    use stripstream_core::schedule::{validate_schedule_mode, mode_to_interval_minutes};

    let push_mode = input.reading_status_push_mode.as_deref().unwrap_or("manual");
    validate_schedule_mode(push_mode)
        .map_err(|e| ApiError::bad_request(format!("reading_status_push_mode {e}")))?;

    let next_push_at = if push_mode != "manual" {
        Some(chrono::Utc::now() + chrono::Duration::minutes(mode_to_interval_minutes(push_mode)))
    } else {
        None
    };

    let result = sqlx::query(
        "UPDATE libraries SET reading_status_provider = $2, reading_status_push_mode = $3, next_reading_status_push_at = $4 WHERE id = $1"
    )
    .bind(library_id)
    .bind(provider)
    .bind(push_mode)
    .bind(next_push_at)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("library not found"));
    }

    Ok(Json(serde_json::json!({
        "reading_status_provider": provider,
        "reading_status_push_mode": push_mode,
    })))
}
