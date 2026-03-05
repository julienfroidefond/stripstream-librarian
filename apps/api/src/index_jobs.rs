use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::ApiError, AppState};

#[derive(Deserialize)]
pub struct RebuildRequest {
    pub library_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct IndexJobItem {
    pub id: Uuid,
    pub library_id: Option<Uuid>,
    pub r#type: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub stats_json: Option<serde_json::Value>,
    pub error_opt: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn enqueue_rebuild(
    State(state): State<AppState>,
    payload: Option<Json<RebuildRequest>>,
) -> Result<Json<IndexJobItem>, ApiError> {
    let library_id = payload.and_then(|p| p.0.library_id);
    let id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'rebuild', 'pending')",
    )
    .bind(id)
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, library_id, type, status, started_at, finished_at, stats_json, error_opt, created_at FROM index_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(map_row(row)))
}

pub async fn list_index_jobs(State(state): State<AppState>) -> Result<Json<Vec<IndexJobItem>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, library_id, type, status, started_at, finished_at, stats_json, error_opt, created_at FROM index_jobs ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows.into_iter().map(map_row).collect()))
}

pub async fn cancel_job(
    State(state): State<AppState>,
    id: axum::extract::Path<Uuid>,
) -> Result<Json<IndexJobItem>, ApiError> {
    let rows_affected = sqlx::query(
        "UPDATE index_jobs SET status = 'cancelled' WHERE id = $1 AND status IN ('pending', 'running')",
    )
    .bind(id.0)
    .execute(&state.pool)
    .await?;

    if rows_affected.rows_affected() == 0 {
        return Err(ApiError::not_found("job not found or already finished"));
    }

    let row = sqlx::query(
        "SELECT id, library_id, type, status, started_at, finished_at, stats_json, error_opt, created_at FROM index_jobs WHERE id = $1",
    )
    .bind(id.0)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(map_row(row)))
}

#[derive(Serialize)]
pub struct FolderItem {
    pub name: String,
    pub path: String,
}

pub async fn list_folders(State(_state): State<AppState>) -> Result<Json<Vec<FolderItem>>, ApiError> {
    let libraries_path = std::path::Path::new("/libraries");
    let mut folders = Vec::new();

    if let Ok(entries) = std::fs::read_dir(libraries_path) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                folders.push(FolderItem {
                    name: name.clone(),
                    path: format!("/libraries/{}", name),
                });
            }
        }
    }

    folders.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(folders))
}

fn map_row(row: sqlx::postgres::PgRow) -> IndexJobItem {
    IndexJobItem {
        id: row.get("id"),
        library_id: row.get("library_id"),
        r#type: row.get("type"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        stats_json: row.get("stats_json"),
        error_opt: row.get("error_opt"),
        created_at: row.get("created_at"),
    }
}
