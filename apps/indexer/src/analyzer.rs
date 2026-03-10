use anyhow::Result;
use futures::stream::{self, StreamExt};
use image::GenericImageView;
use parsers::{analyze_book, BookFormat};
use sqlx::Row;
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{utils, AppState};

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

async fn load_thumbnail_concurrency(pool: &sqlx::PgPool) -> usize {
    let default_concurrency = 2;
    let row = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'limits'"#)
        .fetch_optional(pool)
        .await;

    match row {
        Ok(Some(row)) => {
            let value: serde_json::Value = row.get("value");
            value
                .get("concurrent_renders")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(default_concurrency)
        }
        _ => default_concurrency,
    }
}

fn generate_thumbnail(image_bytes: &[u8], config: &ThumbnailConfig) -> anyhow::Result<Vec<u8>> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| anyhow::anyhow!("failed to load image: {}", e))?;
    let (orig_w, orig_h) = img.dimensions();
    let ratio_w = config.width as f32 / orig_w as f32;
    let ratio_h = config.height as f32 / orig_h as f32;
    let ratio = ratio_w.min(ratio_h);
    let new_w = (orig_w as f32 * ratio) as u32;
    let new_h = (orig_h as f32 * ratio) as u32;
    let resized = img.resize(new_w, new_h, image::imageops::FilterType::Triangle);
    let rgba = resized.to_rgba8();
    let (w, h) = rgba.dimensions();
    let rgb_data: Vec<u8> = rgba.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect();
    let quality = config.quality as f32;
    let webp_data = webp::Encoder::new(&rgb_data, webp::PixelLayout::Rgb, w, h).encode(quality);
    Ok(webp_data.to_vec())
}

fn save_thumbnail(
    book_id: Uuid,
    thumbnail_bytes: &[u8],
    config: &ThumbnailConfig,
) -> anyhow::Result<String> {
    let dir = Path::new(&config.directory);
    std::fs::create_dir_all(dir)?;
    let filename = format!("{}.webp", book_id);
    let path = dir.join(&filename);
    std::fs::write(&path, thumbnail_bytes)?;
    Ok(path.to_string_lossy().to_string())
}

fn book_format_from_str(s: &str) -> Option<BookFormat> {
    match s {
        "cbz" => Some(BookFormat::Cbz),
        "cbr" => Some(BookFormat::Cbr),
        "pdf" => Some(BookFormat::Pdf),
        _ => None,
    }
}

/// Phase 2 — Analysis: open each unanalyzed archive once, extract page_count + thumbnail.
/// `thumbnail_only` = true: only process books missing thumbnail (page_count may already be set).
/// `thumbnail_only` = false: process books missing page_count.
pub async fn analyze_library_books(
    state: &AppState,
    job_id: Uuid,
    library_id: Option<Uuid>,
    thumbnail_only: bool,
) -> Result<()> {
    let config = load_thumbnail_config(&state.pool).await;

    if !config.enabled {
        info!("[ANALYZER] Thumbnails disabled, skipping analysis phase");
        return Ok(());
    }

    let concurrency = load_thumbnail_concurrency(&state.pool).await;

    // Query books that need analysis
    let query_filter = if thumbnail_only {
        "b.thumbnail_path IS NULL"
    } else {
        "b.page_count IS NULL"
    };

    let sql = format!(
        r#"
        SELECT b.id AS book_id, bf.abs_path, bf.format
        FROM books b
        JOIN book_files bf ON bf.book_id = b.id
        WHERE (b.library_id = $1 OR $1 IS NULL)
          AND {}
        "#,
        query_filter
    );

    let rows = sqlx::query(&sql)
        .bind(library_id)
        .fetch_all(&state.pool)
        .await?;

    if rows.is_empty() {
        info!("[ANALYZER] No books to analyze");
        return Ok(());
    }

    let total = rows.len() as i32;
    info!(
        "[ANALYZER] Analyzing {} books (thumbnail_only={}, concurrency={})",
        total, thumbnail_only, concurrency
    );

    // Update job status
    let _ = sqlx::query(
        "UPDATE index_jobs SET status = 'generating_thumbnails', total_files = $2, processed_files = 0, current_file = NULL WHERE id = $1",
    )
    .bind(job_id)
    .bind(total)
    .execute(&state.pool)
    .await;

    let processed_count = Arc::new(AtomicI32::new(0));

    struct BookTask {
        book_id: Uuid,
        abs_path: String,
        format: String,
    }

    let tasks: Vec<BookTask> = rows
        .into_iter()
        .map(|row| BookTask {
            book_id: row.get("book_id"),
            abs_path: row.get("abs_path"),
            format: row.get("format"),
        })
        .collect();

    stream::iter(tasks)
        .for_each_concurrent(concurrency, |task| {
            let processed_count = processed_count.clone();
            let pool = state.pool.clone();
            let config = config.clone();

            async move {
                let local_path = utils::remap_libraries_path(&task.abs_path);
                let path = Path::new(&local_path);

                let format = match book_format_from_str(&task.format) {
                    Some(f) => f,
                    None => {
                        warn!("[ANALYZER] Unknown format '{}' for book {}", task.format, task.book_id);
                        return;
                    }
                };

                // Run blocking archive I/O on a thread pool
                let book_id = task.book_id;
                let path_owned = path.to_path_buf();
                let analyze_result = tokio::task::spawn_blocking(move || {
                    analyze_book(&path_owned, format)
                })
                .await;

                let (page_count, image_bytes) = match analyze_result {
                    Ok(Ok(result)) => result,
                    Ok(Err(e)) => {
                        warn!("[ANALYZER] analyze_book failed for book {}: {}", book_id, e);
                        // Mark parse_status = error in book_files
                        let _ = sqlx::query(
                            "UPDATE book_files SET parse_status = 'error', parse_error_opt = $2 WHERE book_id = $1",
                        )
                        .bind(book_id)
                        .bind(e.to_string())
                        .execute(&pool)
                        .await;
                        return;
                    }
                    Err(e) => {
                        warn!("[ANALYZER] spawn_blocking error for book {}: {}", book_id, e);
                        return;
                    }
                };

                // Generate thumbnail
                let thumb_result = tokio::task::spawn_blocking({
                    let config = config.clone();
                    move || generate_thumbnail(&image_bytes, &config)
                })
                .await;

                let thumb_bytes = match thumb_result {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => {
                        warn!("[ANALYZER] thumbnail generation failed for book {}: {}", book_id, e);
                        // Still update page_count even if thumbnail fails
                        let _ = sqlx::query(
                            "UPDATE books SET page_count = $1 WHERE id = $2",
                        )
                        .bind(page_count)
                        .bind(book_id)
                        .execute(&pool)
                        .await;
                        return;
                    }
                    Err(e) => {
                        warn!("[ANALYZER] spawn_blocking thumbnail error for book {}: {}", book_id, e);
                        return;
                    }
                };

                // Save thumbnail file
                let save_result = {
                    let config = config.clone();
                    tokio::task::spawn_blocking(move || save_thumbnail(book_id, &thumb_bytes, &config))
                        .await
                };

                let thumb_path = match save_result {
                    Ok(Ok(p)) => p,
                    Ok(Err(e)) => {
                        warn!("[ANALYZER] save_thumbnail failed for book {}: {}", book_id, e);
                        let _ = sqlx::query("UPDATE books SET page_count = $1 WHERE id = $2")
                            .bind(page_count)
                            .bind(book_id)
                            .execute(&pool)
                            .await;
                        return;
                    }
                    Err(e) => {
                        warn!("[ANALYZER] spawn_blocking save error for book {}: {}", book_id, e);
                        return;
                    }
                };

                // Update DB
                if let Err(e) = sqlx::query(
                    "UPDATE books SET page_count = $1, thumbnail_path = $2 WHERE id = $3",
                )
                .bind(page_count)
                .bind(&thumb_path)
                .bind(book_id)
                .execute(&pool)
                .await
                {
                    warn!("[ANALYZER] DB update failed for book {}: {}", book_id, e);
                    return;
                }

                let processed = processed_count.fetch_add(1, Ordering::Relaxed) + 1;
                let percent = (processed as f64 / total as f64 * 100.0) as i32;
                let _ = sqlx::query(
                    "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
                )
                .bind(job_id)
                .bind(processed)
                .bind(percent)
                .execute(&pool)
                .await;
            }
        })
        .await;

    let final_count = processed_count.load(Ordering::Relaxed);
    info!(
        "[ANALYZER] Analysis complete: {}/{} books processed",
        final_count, total
    );

    Ok(())
}

/// Clear thumbnail files and DB references for books in scope, then re-analyze.
pub async fn regenerate_thumbnails(
    state: &AppState,
    job_id: Uuid,
    library_id: Option<Uuid>,
) -> Result<()> {
    let config = load_thumbnail_config(&state.pool).await;

    // Delete thumbnail files for all books in scope
    let book_ids_to_clear: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM books WHERE (library_id = $1 OR $1 IS NULL) AND thumbnail_path IS NOT NULL"#,
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut deleted_count = 0usize;
    for book_id in &book_ids_to_clear {
        let filename = format!("{}.webp", book_id);
        let thumbnail_path = Path::new(&config.directory).join(&filename);
        if thumbnail_path.exists() {
            if let Err(e) = std::fs::remove_file(&thumbnail_path) {
                warn!(
                    "[ANALYZER] Failed to delete thumbnail {}: {}",
                    thumbnail_path.display(),
                    e
                );
            } else {
                deleted_count += 1;
            }
        }
    }
    info!(
        "[ANALYZER] Deleted {} thumbnail files for regeneration",
        deleted_count
    );

    // Clear thumbnail_path in DB
    sqlx::query(
        r#"UPDATE books SET thumbnail_path = NULL WHERE (library_id = $1 OR $1 IS NULL)"#,
    )
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    // Re-analyze all books (now thumbnail_path IS NULL for all)
    analyze_library_books(state, job_id, library_id, true).await
}

/// Delete orphaned thumbnail files (books deleted in full_rebuild get new UUIDs).
pub async fn cleanup_orphaned_thumbnails(state: &AppState) -> Result<()> {
    let config = load_thumbnail_config(&state.pool).await;

    // Load ALL book IDs across all libraries — we need the complete set to avoid
    // deleting thumbnails that belong to other libraries during a per-library rebuild.
    let existing_book_ids: std::collections::HashSet<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM books"#,
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let thumbnail_dir = Path::new(&config.directory);
    if !thumbnail_dir.exists() {
        return Ok(());
    }

    let mut deleted_count = 0usize;
    if let Ok(entries) = std::fs::read_dir(thumbnail_dir) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(".webp") {
                    if let Some(book_id_str) = file_name.strip_suffix(".webp") {
                        if let Ok(book_id) = Uuid::parse_str(book_id_str) {
                            if !existing_book_ids.contains(&book_id) {
                                if let Err(e) = std::fs::remove_file(entry.path()) {
                                    warn!(
                                        "Failed to delete orphaned thumbnail {}: {}",
                                        entry.path().display(),
                                        e
                                    );
                                } else {
                                    deleted_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    info!(
        "[ANALYZER] Deleted {} orphaned thumbnail files",
        deleted_count
    );
    Ok(())
}
