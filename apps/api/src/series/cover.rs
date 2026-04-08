use uuid::Uuid;

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
