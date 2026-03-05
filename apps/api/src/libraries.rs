use std::path::{Path, PathBuf};

use axum::{extract::{Path as AxumPath, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::ApiError, AppState};

#[derive(Serialize)]
pub struct LibraryDto {
    pub id: Uuid,
    pub name: String,
    pub root_path: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct CreateLibraryInput {
    pub name: String,
    pub root_path: String,
}

pub async fn list_libraries(State(state): State<AppState>) -> Result<Json<Vec<LibraryDto>>, ApiError> {
    let rows = sqlx::query("SELECT id, name, root_path, enabled FROM libraries ORDER BY created_at DESC")
        .fetch_all(&state.pool)
        .await?;

    let items = rows
        .into_iter()
        .map(|row| LibraryDto {
            id: row.get("id"),
            name: row.get("name"),
            root_path: row.get("root_path"),
            enabled: row.get("enabled"),
        })
        .collect();

    Ok(Json(items))
}

pub async fn create_library(
    State(state): State<AppState>,
    Json(input): Json<CreateLibraryInput>,
) -> Result<Json<LibraryDto>, ApiError> {
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

    Ok(Json(LibraryDto {
        id,
        name: input.name.trim().to_string(),
        root_path,
        enabled: true,
    }))
}

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
