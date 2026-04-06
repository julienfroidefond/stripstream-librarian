use axum::extract::Extension;
use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{auth::AuthUser, error::ApiError, state::AppState};
use super::helpers::resolve_library_id;

#[derive(Deserialize, ToSchema)]
pub struct UpdateSeriesRequest {
    pub new_name: String,
    /// Series-level authors list (stored in series)
    #[serde(default)]
    pub authors: Vec<String>,
    /// Per-book author propagation: absent = keep books unchanged, present = overwrite all books
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Option<String>>,
    /// Per-book language propagation: absent = keep books unchanged, present = overwrite all books
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<Option<String>>,
    pub description: Option<String>,
    #[serde(default)]
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    /// Series status: "ongoing", "ended", "hiatus", "cancelled", or null
    pub status: Option<String>,
    /// Fields locked from external metadata sync
    #[serde(default)]
    pub locked_fields: Option<serde_json::Value>,
}

#[derive(Serialize, ToSchema)]
pub struct UpdateSeriesResponse {
    pub updated: u64,
}

/// Update metadata for all books in a series (deprecated: use PATCH /series/{series_id})
#[deprecated]
#[utoipa::path(
    patch,
    path = "/libraries/{library_id}/series/{series_id}",
    tag = "series (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    request_body = UpdateSeriesRequest,
    responses(
        (status = 200, body = UpdateSeriesResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_series(
    State(state): State<AppState>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateSeriesRequest>,
) -> Result<Json<UpdateSeriesResponse>, ApiError> {
    let new_name = body.new_name.trim().to_string();
    if new_name.is_empty() {
        return Err(ApiError::bad_request("series name cannot be empty"));
    }

    // Verify the series exists
    let old_row = sqlx::query("SELECT name, original_name FROM series WHERE id = $1 AND library_id = $2")
        .bind(series_id)
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;
    let old_name: String = old_row.get("name");

    // author/language: None = absent (keep books unchanged), Some(v) = apply to all books
    let apply_author = body.author.is_some();
    let author_value = body.author.flatten().as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let apply_language = body.language.is_some();
    let language_value = body.language.flatten().as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let description = body.description.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let publishers: Vec<String> = body.publishers.iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    let authors: Vec<String> = body.authors.iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    let locked_fields = body.locked_fields.clone().unwrap_or(serde_json::json!({}));

    // 1. Update books: author/language only if opted-in
    let result = sqlx::query(
        "UPDATE books \
         SET author = CASE WHEN $2 THEN $3 ELSE author END, \
             language = CASE WHEN $4 THEN $5 ELSE language END, \
             updated_at = NOW() \
         WHERE series_id = $1",
    )
    .bind(series_id)
    .bind(apply_author)
    .bind(&author_value)
    .bind(apply_language)
    .bind(&language_value)
    .execute(&state.pool)
    .await?;

    // 2. Update the series row (name, metadata, original_name tracking)
    let is_rename = new_name != old_name;
    let original_name: Option<String> = if is_rename {
        // Use existing original_name if set (chained renames: A->B->C), otherwise use old name
        let existing_original: Option<String> = old_row.get("original_name");
        Some(existing_original.unwrap_or_else(|| old_name.clone()))
    } else {
        None
    };

    sqlx::query(
        r#"
        UPDATE series
        SET name = $2,
            authors = $3,
            description = $4,
            publishers = $5,
            start_year = $6,
            total_volumes = $7,
            status = $8,
            locked_fields = $9,
            book_author = CASE WHEN $10 THEN $11 ELSE book_author END,
            book_language = CASE WHEN $12 THEN $13 ELSE book_language END,
            original_name = COALESCE($14, original_name),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(series_id)
    .bind(&new_name)
    .bind(&authors)
    .bind(&description)
    .bind(&publishers)
    .bind(body.start_year)
    .bind(body.total_volumes)
    .bind(&body.status)
    .bind(&locked_fields)
    .bind(apply_author)
    .bind(&author_value)
    .bind(apply_language)
    .bind(&language_value)
    .bind(&original_name)
    .execute(&state.pool)
    .await?;

    Ok(Json(UpdateSeriesResponse { updated: result.rows_affected() }))
}

/// Update a series by its UUID (resolves library_id internally)
#[utoipa::path(
    patch,
    path = "/series/{series_id}",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    request_body = UpdateSeriesRequest,
    responses(
        (status = 200, body = UpdateSeriesResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn update_series_by_id(
    state: State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
    body: Json<UpdateSeriesRequest>,
) -> Result<Json<UpdateSeriesResponse>, ApiError> {
    let library_id = resolve_library_id(&state.pool, series_id).await?;
    update_series(state, Path((library_id, series_id)), body).await
}

/// Delete an entire series (deprecated: use DELETE /series/{series_id})
#[deprecated]
#[utoipa::path(
    delete,
    path = "/libraries/{library_id}/series/{series_id}",
    tag = "series (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Series deleted"),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_series(
    State(state): State<AppState>,
    _user: Option<Extension<AuthUser>>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    use stripstream_core::paths::remap_libraries_path;

    // Verify the series exists
    let series_row = sqlx::query("SELECT name FROM series WHERE id = $1 AND library_id = $2")
        .bind(series_id)
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;
    let series_name: String = series_row.get("name");

    // Find all books in this series
    let book_rows = sqlx::query(
        "SELECT b.id, b.thumbnail_path, bf.abs_path \
         FROM books b \
         LEFT JOIN book_files bf ON bf.book_id = b.id \
         WHERE b.series_id = $1",
    )
    .bind(series_id)
    .fetch_all(&state.pool)
    .await?;

    // Collect the series directory from the first book's path
    let mut series_dir: Option<String> = None;

    // Delete each book's physical file and thumbnail
    for row in &book_rows {
        let abs_path: Option<String> = row.get("abs_path");
        let thumbnail_path: Option<String> = row.get("thumbnail_path");

        if let Some(ref path) = abs_path {
            let physical = remap_libraries_path(path);
            if series_dir.is_none() {
                if let Some(parent) = std::path::Path::new(&physical).parent() {
                    series_dir = Some(parent.to_string_lossy().into_owned());
                }
            }
            match std::fs::remove_file(&physical) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!("[SERIES] Failed to delete file {}: {}", physical, e);
                }
            }
        }

        if let Some(ref path) = thumbnail_path {
            let _ = std::fs::remove_file(path);
        }
    }

    // Delete the series directory
    // Use dir from book paths if available, otherwise build from library root + series name
    if series_dir.is_none() {
        if let Ok(root) = sqlx::query_scalar::<_, String>("SELECT root_path FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_one(&state.pool)
            .await
        {
            let physical = remap_libraries_path(&root);
            series_dir = Some(
                std::path::Path::new(&physical)
                    .join(&series_name)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }

    if let Some(ref dir) = series_dir {
        let dir_path = std::path::Path::new(dir);
        if dir_path.exists() {
            match std::fs::remove_dir_all(dir) {
                Ok(()) => tracing::info!("[SERIES] Deleted series directory: {}", dir),
                Err(e) => tracing::warn!("[SERIES] Failed to delete series directory {}: {}", dir, e),
            }
        }
    }

    // Delete all books from DB (cascades to book_files, reading_progress, etc.)
    let book_ids: Vec<Uuid> = book_rows.iter().map(|r| r.get("id")).collect();
    if !book_ids.is_empty() {
        sqlx::query("DELETE FROM books WHERE id = ANY($1)")
            .bind(&book_ids)
            .execute(&state.pool)
            .await?;
    }

    // Delete the series row (cascades to external_metadata_links, anilist_series_links, available_downloads via FK)
    sqlx::query("DELETE FROM series WHERE id = $1")
        .bind(series_id)
        .execute(&state.pool)
        .await?;

    // Queue a scan job for consistency
    let scan_job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(scan_job_id)
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "[SERIES] Deleted series '{}' ({}) ({} books) from library {}, scan job {} queued",
        series_name, series_id, book_ids.len(), library_id, scan_job_id
    );

    Ok(Json(crate::responses::DeletedResponse::new(library_id)))
}

/// Delete a series by its UUID (resolves library_id internally)
#[utoipa::path(
    delete,
    path = "/series/{series_id}",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Series deleted"),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn delete_series_by_id(
    state: State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    let library_id = resolve_library_id(&state.pool, series_id).await?;
    delete_series(state, user, Path((library_id, series_id))).await
}
