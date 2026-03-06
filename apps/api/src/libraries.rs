use std::path::{Path, PathBuf};

use axum::{extract::{Path as AxumPath, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, AppState};

#[derive(Serialize, ToSchema)]
pub struct LibraryResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub root_path: String,
    pub enabled: bool,
    pub book_count: i64,
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
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_libraries(State(state): State<AppState>) -> Result<Json<Vec<LibraryResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT l.id, l.name, l.root_path, l.enabled, 
                (SELECT COUNT(*) FROM books b WHERE b.library_id = l.id) as book_count 
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
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("library not found"));
    }

    Ok(Json(serde_json::json!({"deleted": true, "id": id})))
}

fn canonicalize_library_root(root_path: &str) -> Result<PathBuf, ApiError> {
    let path = Path::new(root_path);
    if !path.is_absolute() {
        return Err(ApiError::bad_request("root_path must be absolute"));
    }

    let canonical = std::fs::canonicalize(path)
        .map_err(|_| ApiError::bad_request("root_path does not exist or is inaccessible"))?;

    if !canonical.is_dir() {
        return Err(ApiError::bad_request("root_path must point to a directory"));
    }

    Ok(canonical)
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
        (status = 403, description = "Forbidden - Admin scope required"),
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
    let job_type = if is_full { "full_rebuild" } else { "rebuild" };

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
