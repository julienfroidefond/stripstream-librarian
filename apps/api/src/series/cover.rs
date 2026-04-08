use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use sqlx::Row;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// Download an external cover URL and save it locally.
/// Returns the local file path, or None if download failed.
pub(crate) async fn download_and_store_cover(
    pool: &sqlx::PgPool,
    series_id: Uuid,
    cover_url: &str,
) -> Option<String> {
    if cover_url.is_empty() || cover_url.starts_with('/') {
        // Already a local path or empty
        return None;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;

    let resp = client.get(cover_url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::warn!("[COVER] Failed to download cover for series {}: HTTP {}", series_id, resp.status());
        return None;
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() {
        return None;
    }

    // Determine extension from content type
    let ext = match content_type.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    };

    // Store in thumbnail directory
    let thumbnail_dir = std::env::var("THUMBNAIL_DIRECTORY").unwrap_or_else(|_| "/data/thumbnails".to_string());
    let covers_dir = format!("{}/series_covers", thumbnail_dir);
    if let Err(e) = std::fs::create_dir_all(&covers_dir) {
        tracing::warn!("[COVER] Failed to create covers directory {}: {}", covers_dir, e);
        return None;
    }

    let filename = format!("{}.{}", series_id, ext);
    let local_path = format!("{}/{}", covers_dir, filename);

    if let Err(e) = std::fs::write(&local_path, &bytes) {
        tracing::warn!("[COVER] Failed to write cover file {}: {}", local_path, e);
        return None;
    }

    tracing::info!("[COVER] Downloaded cover for series {} ({} bytes) → {}", series_id, bytes.len(), local_path);

    // Update series.cover_url to local path
    let _ = sqlx::query("UPDATE series SET cover_url = $1 WHERE id = $2")
        .bind(&local_path)
        .bind(series_id)
        .execute(pool)
        .await;

    Some(local_path)
}

/// Serve the series cover image.
/// If the series has a local cover file, serve it. Otherwise return 404.
#[utoipa::path(
    get,
    path = "/series/{series_id}/cover",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Cover image"),
        (status = 404, description = "No cover available"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_cover(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let row = sqlx::query("SELECT cover_url FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("series not found"))?;

    let cover_url: Option<String> = row.get("cover_url");
    let cover_path = cover_url.ok_or_else(|| ApiError::not_found("no cover available"))?;

    // If it's an external URL (not yet downloaded), download it now
    let local_path = if cover_path.starts_with("http://") || cover_path.starts_with("https://") {
        match download_and_store_cover(&state.pool, series_id, &cover_path).await {
            Some(p) => p,
            None => return Err(ApiError::not_found("failed to download cover")),
        }
    } else {
        cover_path
    };

    let data = std::fs::read(&local_path)
        .map_err(|_| ApiError::not_found("cover file not found on disk"))?;

    let content_type = if local_path.ends_with(".png") {
        "image/png"
    } else if local_path.ends_with(".webp") {
        "image/webp"
    } else if local_path.ends_with(".gif") {
        "image/gif"
    } else {
        "image/jpeg"
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );

    Ok((StatusCode::OK, headers, Body::from(data)))
}
