use anyhow::Result;
use futures::stream::{self, StreamExt};
use image::{GenericImageView, ImageEncoder};
use parsers::{analyze_book, BookFormat};
use sqlx::Row;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{batch::EventInsert, job::is_job_cancelled, utils, AppState};

#[derive(Clone)]
struct ThumbnailConfig {
    enabled: bool,
    format: Option<String>,
    width: u32,
    height: u32,
    quality: u8,
    directory: String,
    timeout_secs: u64,
}

async fn load_thumbnail_config(pool: &sqlx::PgPool) -> ThumbnailConfig {
    let fallback = ThumbnailConfig {
        enabled: true,
        format: Some("webp".to_string()),
        width: 300,
        height: 400,
        quality: 80,
        directory: "/data/thumbnails".to_string(),
        timeout_secs: 120,
    };
    let thumb_row = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'thumbnail'"#)
        .fetch_optional(pool)
        .await;
    let limits_row = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'limits'"#)
        .fetch_optional(pool)
        .await;

    let timeout_secs = limits_row
        .ok()
        .flatten()
        .and_then(|r| r.get::<serde_json::Value, _>("value").get("timeout_seconds").and_then(|v| v.as_u64()))
        .unwrap_or(fallback.timeout_secs);

    match thumb_row {
        Ok(Some(row)) => {
            let value: serde_json::Value = row.get("value");
            ThumbnailConfig {
                enabled: value
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(fallback.enabled),
                format: value
                    .get("format")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| fallback.format.clone()),
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
                timeout_secs,
            }
        }
        _ => ThumbnailConfig { timeout_secs, ..fallback },
    }
}

async fn load_thumbnail_concurrency(pool: &sqlx::PgPool) -> usize {
    // Default: half the logical CPUs, clamped between 2 and 8.
    // Archive extraction is I/O bound but benefits from moderate parallelism.
    let cpus = num_cpus::get();
    let default_concurrency = (cpus / 2).clamp(1, 2);
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

/// Detect the image format from raw bytes and return the corresponding file extension.
fn detect_image_ext(data: &[u8]) -> &'static str {
    match image::guess_format(data) {
        Ok(image::ImageFormat::Png) => "png",
        Ok(image::ImageFormat::WebP) => "webp",
        _ => "jpg", // JPEG is the most common in comic archives
    }
}

/// Fast JPEG decode with DCT scaling: decodes directly at reduced resolution (1/8, 1/4, 1/2).
/// Returns (DynamicImage, original_width, original_height) or None if not JPEG / decode fails.
fn fast_jpeg_decode(image_bytes: &[u8], target_w: u32, target_h: u32) -> Option<(image::DynamicImage, u32, u32)> {
    // Only attempt for JPEG
    if image::guess_format(image_bytes).ok()? != image::ImageFormat::Jpeg {
        return None;
    }

    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(image_bytes));
    // Read header to get original dimensions
    decoder.read_info().ok()?;
    let info = decoder.info()?;
    let orig_w = info.width as u32;
    let orig_h = info.height as u32;

    // Request DCT-scaled decode (picks smallest scale >= requested size)
    decoder.scale(target_w as u16, target_h as u16).ok()?;

    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let dec_w = info.width as u32;
    let dec_h = info.height as u32;

    let img = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            let buf = image::RgbImage::from_raw(dec_w, dec_h, pixels)?;
            image::DynamicImage::ImageRgb8(buf)
        }
        jpeg_decoder::PixelFormat::L8 => {
            let buf = image::GrayImage::from_raw(dec_w, dec_h, pixels)?;
            image::DynamicImage::ImageLuma8(buf)
        }
        _ => return None,
    };
    Some((img, orig_w, orig_h))
}

fn generate_thumbnail(image_bytes: &[u8], config: &ThumbnailConfig) -> anyhow::Result<Vec<u8>> {
    let t0 = std::time::Instant::now();

    // Try fast JPEG DCT-scaled decode first (decodes directly at ~target size)
    let (img, orig_w, orig_h) = if let Some(result) = fast_jpeg_decode(image_bytes, config.width, config.height) {
        result
    } else {
        // Fallback for PNG/WebP/other formats
        let img = image::load_from_memory(image_bytes)
            .map_err(|e| anyhow::anyhow!("failed to load image: {}", e))?;
        let (ow, oh) = img.dimensions();
        (img, ow, oh)
    };
    let t_decode = t0.elapsed();

    // Don't upscale — clamp to original size
    let target_w = config.width.min(orig_w);
    let target_h = config.height.min(orig_h);

    let t1 = std::time::Instant::now();
    // thumbnail() is optimized for large downscale ratios (uses fast sampling)
    let resized = img.thumbnail(target_w, target_h);
    let (w, h) = resized.dimensions();
    let t_resize = t1.elapsed();

    let format = config.format.as_deref().unwrap_or("webp");
    debug!(
        target: "thumbnail",
        "[THUMBNAIL] {}x{} -> {}x{}  decode={:.0}ms  resize={:.0}ms  encode_format={}",
        orig_w, orig_h, w, h,
        t_decode.as_secs_f64() * 1000.0,
        t_resize.as_secs_f64() * 1000.0,
        format,
    );

    let t2 = std::time::Instant::now();
    let result = match format {
        "original" => {
            // Re-encode in source format (fast JPEG encode instead of slow WebP)
            let source_format = image::guess_format(image_bytes).unwrap_or(image::ImageFormat::Jpeg);
            match source_format {
                image::ImageFormat::Png => {
                    let rgba = resized.to_rgba8();
                    let mut buf = Vec::new();
                    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
                    encoder.write_image(&rgba, w, h, image::ColorType::Rgba8.into())
                        .map_err(|e| anyhow::anyhow!("png encode failed: {}", e))?;
                    Ok(buf)
                }
                _ => {
                    let rgb = resized.to_rgb8();
                    let mut buf = Vec::new();
                    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, config.quality);
                    encoder.encode(&rgb, w, h, image::ColorType::Rgb8.into())
                        .map_err(|e| anyhow::anyhow!("jpeg encode failed: {}", e))?;
                    Ok(buf)
                }
            }
        }
        "jpeg" | "jpg" => {
            let rgb = resized.to_rgb8();
            let mut buf = Vec::new();
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, config.quality);
            encoder.encode(&rgb, w, h, image::ColorType::Rgb8.into())
                .map_err(|e| anyhow::anyhow!("jpeg encode failed: {}", e))?;
            Ok(buf)
        }
        "png" => {
            let rgba = resized.to_rgba8();
            let mut buf = Vec::new();
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            encoder.write_image(&rgba, w, h, image::ColorType::Rgba8.into())
                .map_err(|e| anyhow::anyhow!("png encode failed: {}", e))?;
            Ok(buf)
        }
        _ => {
            // WebP (default)
            let rgb = resized.to_rgb8();
            let rgb_data: &[u8] = rgb.as_raw();
            let quality = config.quality as f32;
            let webp_data = webp::Encoder::new(rgb_data, webp::PixelLayout::Rgb, w, h).encode(quality);
            Ok(webp_data.to_vec())
        }
    };
    let t_encode = t2.elapsed();
    debug!(
        target: "thumbnail",
        "[THUMBNAIL] encode={:.0}ms  total={:.0}ms  output_size={}KB",
        t_encode.as_secs_f64() * 1000.0,
        t0.elapsed().as_secs_f64() * 1000.0,
        result.as_ref().map(|b| b.len() / 1024).unwrap_or(0),
    );
    result
}

/// Save raw image bytes (as extracted from the archive) without any processing.
fn save_raw_image(book_id: Uuid, raw_bytes: &[u8], directory: &str) -> anyhow::Result<String> {
    let dir = Path::new(directory);
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.raw", book_id));
    std::fs::write(&path, raw_bytes)?;
    Ok(path.to_string_lossy().to_string())
}

/// Resize the raw image and save it as a thumbnail, overwriting the raw file.
fn resize_raw_to_thumbnail(
    book_id: Uuid,
    raw_path: &str,
    config: &ThumbnailConfig,
) -> anyhow::Result<String> {
    let raw_bytes = std::fs::read(raw_path)
        .map_err(|e| anyhow::anyhow!("failed to read raw image {}: {}", raw_path, e))?;
    debug!(target: "thumbnail", "[THUMBNAIL] book={} raw_size={}KB", book_id, raw_bytes.len() / 1024);
    let thumb_bytes = generate_thumbnail(&raw_bytes, config)?;

    let format = config.format.as_deref().unwrap_or("webp");
    let ext = match format {
        "original" => detect_image_ext(&raw_bytes),
        "jpeg" | "jpg" => "jpg",
        "png" => "png",
        _ => "webp",
    };

    let thumb_path = Path::new(&config.directory).join(format!("{}.{}", book_id, ext));
    std::fs::write(&thumb_path, &thumb_bytes)?;

    // Delete the raw file now that the thumbnail is written
    let _ = std::fs::remove_file(raw_path);

    Ok(thumb_path.to_string_lossy().to_string())
}

/// Insert a single event into the index_job_events table.
async fn insert_event(pool: &sqlx::PgPool, event: &EventInsert) {
    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_id, entity_name, message, detail)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(event.job_id)
    .bind(&event.event_type)
    .bind(&event.level)
    .bind(&event.entity_type)
    .bind(event.entity_id)
    .bind(&event.entity_name)
    .bind(&event.message)
    .bind(&event.detail)
    .execute(pool)
    .await
    {
        warn!("[ANALYZER] Failed to insert event: {}", e);
    }
}

fn book_format_from_str(s: &str) -> Option<BookFormat> {
    match s {
        "cbz" => Some(BookFormat::Cbz),
        "cbr" => Some(BookFormat::Cbr),
        "pdf" => Some(BookFormat::Pdf),
        "epub" => Some(BookFormat::Epub),
        _ => None,
    }
}

/// Phase 2 — Two-sub-phase analysis:
///
/// **Sub-phase A (extracting_pages)**: open each archive once, extract (page_count, raw_image_bytes),
/// save the raw bytes to `{directory}/{book_id}.raw`. I/O bound — runs at `concurrent_renders`.
///
/// **Sub-phase B (generating_thumbnails)**: load each `.raw` file, resize and encode as WebP,
/// overwrite as `{directory}/{book_id}.webp`. CPU bound — runs at `concurrent_renders`.
///
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

    let query_filter = if thumbnail_only {
        "b.thumbnail_path IS NULL"
    } else {
        "b.page_count IS NULL"
    };

    let sql = format!(
        r#"
        SELECT b.id AS book_id, bf.abs_path, bf.format, (b.thumbnail_path IS NULL) AS needs_thumbnail
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

    let (cancelled_flag, cancel_handle) = spawn_cancellation_poller(state.pool.clone(), job_id);

    #[derive(Clone)]
    struct BookTask {
        book_id: Uuid,
        abs_path: String,
        format: String,
        needs_thumbnail: bool,
    }

    let tasks: Vec<BookTask> = rows
        .into_iter()
        .map(|row| BookTask {
            book_id: row.get("book_id"),
            abs_path: row.get("abs_path"),
            format: row.get("format"),
            needs_thumbnail: row.get("needs_thumbnail"),
        })
        .collect();

    // -------------------------------------------------------------------------
    // Sub-phase A: extract first page from each archive and store raw image
    // Processed in batches of 500 to limit memory — raw_bytes are freed between batches.
    // The collected results (Uuid, String, i32) are lightweight (~100 bytes each).
    // -------------------------------------------------------------------------
    const BATCH_SIZE: usize = 200;

    let phase_a_start = std::time::Instant::now();
    if let Err(e) = sqlx::query(
        "UPDATE index_jobs SET status = 'extracting_pages', total_files = $2, processed_files = 0, current_file = NULL WHERE id = $1",
    )
    .bind(job_id)
    .bind(total)
    .execute(&state.pool)
    .await {
        warn!("[ANALYZER] Failed to update job status to extracting_pages: {e}");
    }

    let extracted_count = Arc::new(AtomicI32::new(0));
    let mut all_extracted: Vec<(Uuid, String, i32)> = Vec::new();

    let num_batches = (tasks.len() + BATCH_SIZE - 1) / BATCH_SIZE;
    let task_chunks: Vec<Vec<BookTask>> = tasks
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(BATCH_SIZE)
        .map(|c| c.to_vec())
        .collect();

    for (batch_idx, batch_tasks) in task_chunks.into_iter().enumerate() {
        if cancelled_flag.load(Ordering::Relaxed) {
            break;
        }

        info!(
            "[ANALYZER] Extraction batch {}/{} — {} books",
            batch_idx + 1, num_batches, batch_tasks.len()
        );

        let batch_extracted: Vec<(Uuid, String, i32)> = stream::iter(batch_tasks)
            .map(|task| {
                let pool = state.pool.clone();
                let config = config.clone();
                let cancelled = cancelled_flag.clone();
                let extracted_count = extracted_count.clone();

                async move {
                    if cancelled.load(Ordering::Relaxed) {
                        return None;
                    }

                    let local_path = utils::remap_libraries_path(&task.abs_path);
                    let path = std::path::Path::new(&local_path);
                    let book_id = task.book_id;
                    let needs_thumbnail = task.needs_thumbnail;

                    // Remove macOS Apple Double resource fork files (._*) that were indexed before the scanner filter was added
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("._"))
                        .unwrap_or(false)
                    {
                        warn!("[ANALYZER] Removing macOS resource fork from DB: {}", local_path);
                        let _ = sqlx::query("DELETE FROM book_files WHERE book_id = $1")
                            .bind(book_id)
                            .execute(&pool)
                            .await;
                        let _ = sqlx::query(
                            "DELETE FROM books WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = $1)",
                        )
                        .bind(book_id)
                        .execute(&pool)
                        .await;
                        return None;
                    }

                    let format = match book_format_from_str(&task.format) {
                        Some(f) => f,
                        None => {
                            warn!("[ANALYZER] Unknown format '{}' for book {}", task.format, book_id);
                            return None;
                        }
                    };

                    let pdf_scale = config.width.max(config.height);
                    let path_owned = path.to_path_buf();
                    let timeout_secs = config.timeout_secs;
                    let file_name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| local_path.clone());

                    debug!(target: "extraction", "[EXTRACTION] Starting: {} ({})", file_name, task.format);
                    let extract_start = std::time::Instant::now();

                    let analyze_result = tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        tokio::task::spawn_blocking(move || analyze_book(&path_owned, format, pdf_scale)),
                    )
                    .await;

                    let (page_count, raw_bytes) = match analyze_result {
                        Ok(Ok(Ok(result))) => result,
                        Ok(Ok(Err(e))) => {
                            warn!(target: "extraction", "[EXTRACTION] Failed: {} — {}", file_name, e);
                            let _ = sqlx::query(
                                "UPDATE book_files SET parse_status = 'error', parse_error_opt = $2 WHERE book_id = $1",
                            )
                            .bind(book_id)
                            .bind(e.to_string())
                            .execute(&pool)
                            .await;
                            insert_event(&pool, &EventInsert {
                                job_id,
                                event_type: "error".to_string(),
                                level: "error".to_string(),
                                entity_type: Some("book".to_string()),
                                entity_id: Some(book_id),
                                entity_name: Some(local_path.clone()),
                                message: Some(format!("Extraction failed: {}", e)),
                                detail: None,
                            }).await;
                            return None;
                        }
                        Ok(Err(e)) => {
                            warn!(target: "extraction", "[EXTRACTION] spawn error: {} — {}", file_name, e);
                            insert_event(&pool, &EventInsert {
                                job_id,
                                event_type: "error".to_string(),
                                level: "error".to_string(),
                                entity_type: Some("book".to_string()),
                                entity_id: Some(book_id),
                                entity_name: Some(local_path.clone()),
                                message: Some(format!("Spawn error: {}", e)),
                                detail: None,
                            }).await;
                            return None;
                        }
                        Err(_) => {
                            warn!(target: "extraction", "[EXTRACTION] Timeout ({}s): {}", timeout_secs, file_name);
                            let _ = sqlx::query(
                                "UPDATE book_files SET parse_status = 'error', parse_error_opt = $2 WHERE book_id = $1",
                            )
                            .bind(book_id)
                            .bind(format!("analyze_book timed out after {}s", timeout_secs))
                            .execute(&pool)
                            .await;
                            insert_event(&pool, &EventInsert {
                                job_id,
                                event_type: "error".to_string(),
                                level: "error".to_string(),
                                entity_type: Some("book".to_string()),
                                entity_id: Some(book_id),
                                entity_name: Some(local_path.clone()),
                                message: Some(format!("Extraction timed out after {}s", timeout_secs)),
                                detail: None,
                            }).await;
                            return None;
                        }
                    };

                    let extract_elapsed = extract_start.elapsed();
                    debug!(
                        target: "extraction",
                        "[EXTRACTION] Done: {} — {} pages, image={}KB in {:.0}ms",
                        file_name, page_count, raw_bytes.len() / 1024,
                        extract_elapsed.as_secs_f64() * 1000.0,
                    );

                    // If thumbnail already exists, just update page_count and skip thumbnail generation
                    if !needs_thumbnail {
                        debug!(target: "extraction", "[EXTRACTION] Page count only: {} — {} pages", file_name, page_count);
                        if let Err(e) = sqlx::query("UPDATE books SET page_count = $1 WHERE id = $2")
                            .bind(page_count)
                            .bind(book_id)
                            .execute(&pool)
                            .await
                        {
                            warn!(target: "extraction", "[EXTRACTION] DB page_count update failed for {}: {}", file_name, e);
                        }
                        insert_event(&pool, &EventInsert {
                            job_id,
                            event_type: "pages_extracted".to_string(),
                            level: "info".to_string(),
                            entity_type: Some("book".to_string()),
                            entity_id: Some(book_id),
                            entity_name: Some(local_path.clone()),
                            message: Some(format!("Extracted {} pages", page_count)),
                            detail: Some(serde_json::json!({"page_count": page_count})),
                        }).await;
                        let processed = extracted_count.fetch_add(1, Ordering::Relaxed) + 1;
                        let percent = (processed as f64 / total as f64 * 50.0) as i32;
                        if let Err(e) = sqlx::query(
                            "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
                        )
                        .bind(job_id)
                        .bind(processed)
                        .bind(percent)
                        .execute(&pool)
                        .await {
                            warn!("[ANALYZER] Failed to update job progress: {e}");
                        }

                        if processed % 25 == 0 || processed == total {
                            info!(
                                target: "extraction",
                                "[EXTRACTION] Progress: {}/{} books extracted ({}%)",
                                processed, total, percent
                            );
                        }
                        return None; // don't enqueue for thumbnail sub-phase
                    }

                    // Save raw bytes to disk (no resize, no encode) — moves raw_bytes, no clone
                    let raw_path = match tokio::task::spawn_blocking({
                        let dir = config.directory.clone();
                        move || save_raw_image(book_id, &raw_bytes, &dir)
                    })
                    .await
                    {
                        Ok(Ok(p)) => p,
                        Ok(Err(e)) => {
                            warn!("[ANALYZER] save_raw_image failed for book {}: {}", book_id, e);
                            return None;
                        }
                        Err(e) => {
                            warn!("[ANALYZER] spawn_blocking save_raw error for book {}: {}", book_id, e);
                            return None;
                        }
                    };

                    // Update page_count in DB
                    if let Err(e) = sqlx::query("UPDATE books SET page_count = $1 WHERE id = $2")
                        .bind(page_count)
                        .bind(book_id)
                        .execute(&pool)
                        .await
                    {
                        warn!("[ANALYZER] DB page_count update failed for book {}: {}", book_id, e);
                        return None;
                    }

                    insert_event(&pool, &EventInsert {
                        job_id,
                        event_type: "pages_extracted".to_string(),
                        level: "info".to_string(),
                        entity_type: Some("book".to_string()),
                        entity_id: Some(book_id),
                        entity_name: Some(local_path.clone()),
                        message: Some(format!("Extracted {} pages", page_count)),
                        detail: Some(serde_json::json!({"page_count": page_count})),
                    }).await;

                    let processed = extracted_count.fetch_add(1, Ordering::Relaxed) + 1;
                    let percent = (processed as f64 / total as f64 * 50.0) as i32; // first 50%
                    if let Err(e) = sqlx::query(
                        "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
                    )
                    .bind(job_id)
                    .bind(processed)
                    .bind(percent)
                    .execute(&pool)
                    .await {
                        warn!("[ANALYZER] Failed to update job progress: {e}");
                    }

                    if processed % 25 == 0 || processed == total {
                        info!(
                            target: "extraction",
                            "[EXTRACTION] Progress: {}/{} books extracted ({}%)",
                            processed, total, percent
                        );
                    }

                    Some((book_id, raw_path, page_count))
                }
            })
            .buffer_unordered(concurrency)
            .filter_map(|x| async move { x })
            .collect()
            .await;

        // Collect lightweight results; raw_bytes already saved to disk and freed
        all_extracted.extend(batch_extracted);

        // Log RSS to track memory growth between batches
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    info!("[ANALYZER] Memory after batch {}/{}: {}", batch_idx + 1, num_batches, line.trim());
                    break;
                }
            }
        }
    }

    if cancelled_flag.load(Ordering::Relaxed) {
        cancel_handle.abort();
        info!("[ANALYZER] Job {} cancelled during extraction phase", job_id);
        return Err(anyhow::anyhow!("Job cancelled by user"));
    }

    let extracted_total = all_extracted.len() as i32;
    let phase_a_elapsed = phase_a_start.elapsed();
    info!(
        "[ANALYZER] Sub-phase A complete: {}/{} books extracted in {:.1}s ({:.0} ms/book, {} batches)",
        extracted_total,
        total,
        phase_a_elapsed.as_secs_f64(),
        if extracted_total > 0 { phase_a_elapsed.as_millis() as f64 / extracted_total as f64 } else { 0.0 },
        num_batches,
    );

    // -------------------------------------------------------------------------
    // Sub-phase B: resize raw images and encode as WebP
    // CPU bound — can run at higher concurrency than I/O phase
    // -------------------------------------------------------------------------
    let phase_b_start = std::time::Instant::now();
    if let Err(e) = sqlx::query(
        "UPDATE index_jobs SET status = 'generating_thumbnails', generating_thumbnails_started_at = NOW(), total_files = $2, processed_files = 0, current_file = NULL WHERE id = $1",
    )
    .bind(job_id)
    .bind(extracted_total)
    .execute(&state.pool)
    .await {
        warn!("[ANALYZER] Failed to update job status to generating_thumbnails: {e}");
    }

    let resize_count = Arc::new(AtomicI32::new(0));

    stream::iter(all_extracted)
        .for_each_concurrent(concurrency, |(book_id, raw_path, page_count)| {
            let pool = state.pool.clone();
            let config = config.clone();
            let cancelled = cancelled_flag.clone();
            let resize_count = resize_count.clone();

            async move {
                if cancelled.load(Ordering::Relaxed) {
                    return;
                }

                let raw_path_clone = raw_path.clone();
                let thumb_result = tokio::task::spawn_blocking(move || {
                    resize_raw_to_thumbnail(book_id, &raw_path_clone, &config)
                })
                .await;

                let thumb_path = match thumb_result {
                    Ok(Ok(p)) => p,
                    Ok(Err(e)) => {
                        warn!("[ANALYZER] resize_raw_to_webp failed for book {}: {}", book_id, e);
                        // page_count is already set; thumbnail stays NULL
                        return;
                    }
                    Err(e) => {
                        warn!("[ANALYZER] spawn_blocking resize error for book {}: {}", book_id, e);
                        return;
                    }
                };

                if let Err(e) = sqlx::query(
                    "UPDATE books SET page_count = $1, thumbnail_path = $2 WHERE id = $3",
                )
                .bind(page_count)
                .bind(&thumb_path)
                .bind(book_id)
                .execute(&pool)
                .await
                {
                    warn!("[ANALYZER] DB thumbnail update failed for book {}: {}", book_id, e);
                    return;
                }

                insert_event(&pool, &EventInsert {
                    job_id,
                    event_type: "thumbnail_generated".to_string(),
                    level: "info".to_string(),
                    entity_type: Some("book".to_string()),
                    entity_id: Some(book_id),
                    entity_name: None,
                    message: Some(format!("Thumbnail generated: {}", thumb_path)),
                    detail: None,
                }).await;

                let processed = resize_count.fetch_add(1, Ordering::Relaxed) + 1;
                let percent =
                    50 + (processed as f64 / extracted_total as f64 * 50.0) as i32; // last 50%
                if let Err(e) = sqlx::query(
                    "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
                )
                .bind(job_id)
                .bind(processed)
                .bind(percent)
                .execute(&pool)
                .await {
                    warn!("[ANALYZER] Failed to update job progress: {e}");
                }

                if processed % 25 == 0 || processed == extracted_total {
                    info!(
                        target: "thumbnail",
                        "[THUMBNAIL] Progress: {}/{} thumbnails generated ({}%)",
                        processed, extracted_total, percent
                    );
                }
            }
        })
        .await;

    cancel_handle.abort();

    if cancelled_flag.load(Ordering::Relaxed) {
        info!("[ANALYZER] Job {} cancelled during resize phase", job_id);
        return Err(anyhow::anyhow!("Job cancelled by user"));
    }

    let final_count = resize_count.load(Ordering::Relaxed);
    let phase_b_elapsed = phase_b_start.elapsed();
    info!(
        "[ANALYZER] Sub-phase B complete: {}/{} thumbnails generated in {:.1}s ({:.0} ms/book)",
        final_count,
        extracted_total,
        phase_b_elapsed.as_secs_f64(),
        if final_count > 0 { phase_b_elapsed.as_millis() as f64 / final_count as f64 } else { 0.0 }
    );
    info!(
        "[ANALYZER] Total: {:.1}s (extraction {:.1}s + resize {:.1}s)",
        (phase_a_elapsed + phase_b_elapsed).as_secs_f64(),
        phase_a_elapsed.as_secs_f64(),
        phase_b_elapsed.as_secs_f64(),
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

    let book_ids_to_clear: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM books WHERE (library_id = $1 OR $1 IS NULL) AND thumbnail_path IS NOT NULL"#,
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut deleted_count = 0usize;
    for book_id in &book_ids_to_clear {
        // Delete thumbnail in any format (webp, jpg, png) + raw
        for ext in &["webp", "jpg", "png", "raw"] {
            let path = Path::new(&config.directory).join(format!("{}.{}", book_id, ext));
            if path.exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    warn!("[ANALYZER] Failed to delete thumbnail {}: {}", path.display(), e);
                } else if *ext != "raw" {
                    deleted_count += 1;
                }
            }
        }
    }
    info!("[ANALYZER] Deleted {} thumbnail files for regeneration", deleted_count);

    sqlx::query(r#"UPDATE books SET thumbnail_path = NULL WHERE (library_id = $1 OR $1 IS NULL)"#)
        .bind(library_id)
        .execute(&state.pool)
        .await?;

    analyze_library_books(state, job_id, library_id, true).await
}

/// Delete orphaned thumbnail files (books deleted in full_rebuild get new UUIDs).
pub async fn cleanup_orphaned_thumbnails(state: &AppState) -> Result<()> {
    let config = load_thumbnail_config(&state.pool).await;

    let existing_book_ids: std::collections::HashSet<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM books"#)
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
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            // Clean up all thumbnail formats and orphaned .raw files
            let stem = [".webp", ".jpg", ".png", ".raw"]
                .iter()
                .find_map(|ext| file_name.strip_suffix(ext).map(|s| s.to_string()));
            if let Some(book_id_str) = stem {
                if let Ok(book_id) = Uuid::parse_str(&book_id_str) {
                    if !existing_book_ids.contains(&book_id) {
                        if let Err(e) = std::fs::remove_file(entry.path()) {
                            warn!("Failed to delete orphaned file {}: {}", entry.path().display(), e);
                        } else {
                            deleted_count += 1;
                        }
                    }
                }
            }
        }
    }

    info!("[ANALYZER] Deleted {} orphaned thumbnail files", deleted_count);
    Ok(())
}

/// Spawn a background task that polls for job cancellation every 2 seconds.
/// Returns the shared cancellation flag and the task handle.
fn spawn_cancellation_poller(
    pool: sqlx::PgPool,
    job_id: Uuid,
) -> (Arc<AtomicBool>, tokio::task::JoinHandle<()>) {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = flag.clone();
    let handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            match is_job_cancelled(&pool, job_id).await {
                Ok(true) => {
                    flag_clone.store(true, Ordering::Relaxed);
                    break;
                }
                Ok(false) => {}
                Err(_) => break,
            }
        }
    });
    (flag, handle)
}
