use std::path::Path;

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use image::GenericImageView;
use serde::Deserialize;
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, index_jobs, pages, AppState};

#[derive(Clone)]
struct ThumbnailConfig {
    enabled: bool,
    width: u32,
    height: u32,
    quality: u8,
    directory: String,
}

async fn load_thumbnail_config(pool: &sqlx::PgPool) -> ThumbnailConfig {
    let fallback = ThumbnailConfig {
        enabled: true,
        width: 300,
        height: 400,
        quality: 80,
        directory: "/data/thumbnails".to_string(),
    };
    let row = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'thumbnail'"#)
        .fetch_optional(pool)
        .await;

    match row {
        Ok(Some(row)) => {
            let value: serde_json::Value = row.get("value");
            ThumbnailConfig {
                enabled: value
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(fallback.enabled),
                width: value
                    .get("width")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
                    .unwrap_or(fallback.width),
                height: value
                    .get("height")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
                    .unwrap_or(fallback.height),
                quality: value
                    .get("quality")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8)
                    .unwrap_or(fallback.quality),
                directory: value
                    .get("directory")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| fallback.directory.clone()),
            }
        }
        _ => fallback,
    }
}

fn generate_thumbnail(image_bytes: &[u8], config: &ThumbnailConfig) -> anyhow::Result<Vec<u8>> {
    let img = image::load_from_memory(image_bytes).context("failed to load image")?;
    let (orig_w, orig_h) = img.dimensions();
    let ratio_w = config.width as f32 / orig_w as f32;
    let ratio_h = config.height as f32 / orig_h as f32;
    let ratio = ratio_w.min(ratio_h);
    let new_w = (orig_w as f32 * ratio) as u32;
    let new_h = (orig_h as f32 * ratio) as u32;
    let resized = img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (w, h) = rgba.dimensions();
    let rgb_data: Vec<u8> = rgba.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect();
    let quality = f32::max(config.quality as f32, 85.0);
    let webp_data =
        webp::Encoder::new(&rgb_data, webp::PixelLayout::Rgb, w, h).encode(quality);
    Ok(webp_data.to_vec())
}

fn save_thumbnail(book_id: Uuid, thumbnail_bytes: &[u8], config: &ThumbnailConfig) -> anyhow::Result<String> {
    let dir = Path::new(&config.directory);
    std::fs::create_dir_all(dir)?;
    let filename = format!("{}.webp", book_id);
    let path = dir.join(&filename);
    std::fs::write(&path, thumbnail_bytes)?;
    Ok(path.to_string_lossy().to_string())
}

async fn run_checkup(state: AppState, job_id: Uuid) {
    let pool = &state.pool;
    let row = sqlx::query("SELECT library_id, type FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(pool)
        .await;

    let (library_id, job_type) = match row {
        Ok(Some(r)) => (
            r.get::<Option<Uuid>, _>("library_id"),
            r.get::<String, _>("type"),
        ),
        _ => {
            warn!("thumbnails checkup: job {} not found", job_id);
            return;
        }
    };

    // Regenerate: clear existing thumbnails in scope so they get regenerated
    if job_type == "thumbnail_regenerate" {
        let cleared = sqlx::query(
            r#"UPDATE books SET thumbnail_path = NULL WHERE (library_id = $1 OR $1 IS NULL)"#,
        )
        .bind(library_id)
        .execute(pool)
        .await;
        if let Ok(res) = cleared {
            info!("thumbnails regenerate: cleared {} books", res.rows_affected());
        }
    }

    let book_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM books WHERE (library_id = $1 OR $1 IS NULL) AND thumbnail_path IS NULL"#,
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let config = load_thumbnail_config(pool).await;
    if !config.enabled || book_ids.is_empty() {
        let _ = sqlx::query(
            "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, current_file = NULL WHERE id = $1",
        )
        .bind(job_id)
        .execute(pool)
        .await;
        return;
    }

    let total = book_ids.len() as i32;
    let _ = sqlx::query(
        "UPDATE index_jobs SET status = 'generating_thumbnails', total_files = $2, processed_files = 0, current_file = NULL WHERE id = $1",
    )
    .bind(job_id)
    .bind(total)
    .execute(pool)
    .await;

    for (i, &book_id) in book_ids.iter().enumerate() {
        match pages::render_book_page_1(&state, book_id, config.width, config.quality).await {
            Ok(page_bytes) => {
                match generate_thumbnail(&page_bytes, &config) {
                    Ok(thumb_bytes) => {
                        if let Ok(path) = save_thumbnail(book_id, &thumb_bytes, &config) {
                            if sqlx::query("UPDATE books SET thumbnail_path = $1 WHERE id = $2")
                                .bind(&path)
                                .bind(book_id)
                                .execute(pool)
                                .await
                                .is_ok()
                            {
                                let processed = (i + 1) as i32;
                                let percent = ((i + 1) as f64 / total as f64 * 100.0) as i32;
                                let _ = sqlx::query(
                                    "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
                                )
                                .bind(job_id)
                                .bind(processed)
                                .bind(percent)
                                .execute(pool)
                                .await;
                            }
                        }
                    }
                    Err(e) => warn!("thumbnail generate failed for book {}: {:?}", book_id, e),
                }
            }
            Err(e) => warn!("render page 1 failed for book {}: {:?}", book_id, e),
        }
    }

    let _ = sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, current_file = NULL WHERE id = $1",
    )
    .bind(job_id)
    .execute(pool)
    .await;

    info!("thumbnails checkup finished for job {} ({} books)", job_id, total);
}

#[derive(Deserialize, ToSchema)]
pub struct ThumbnailsRebuildRequest {
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
}

/// POST /index/thumbnails/rebuild — create a job and generate thumbnails for books that don't have one (optional library scope).
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

/// POST /index/thumbnails/regenerate — create a job and regenerate all thumbnails in scope (clears then regenerates).
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

/// POST /index/jobs/:id/thumbnails/checkup — start thumbnail generation for books missing thumbnails (called by indexer at end of build).
pub async fn start_checkup(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    let state = state.clone();
    tokio::spawn(async move { run_checkup(state, job_id).await });
    Ok(StatusCode::ACCEPTED)
}
