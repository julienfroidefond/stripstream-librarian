use anyhow::Result;
use futures::stream::{self, StreamExt};
use image::{GenericImageView, ImageEncoder};
use parsers::{analyze_book, page_count_only, BookFormat};
use sqlx::Row;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    batch::{flush_events, flush_page_counts, flush_parse_errors, flush_thumbnails, EventInsert},
    job::is_job_cancelled,
    utils, AppState,
};

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

/// Result of extracting one book, collected per batch so DB writes can be batched.
enum ExtractOutcome {
    Skipped,
    ResourceFork {
        book_id: Uuid,
    },
    Error {
        book_id: Uuid,
        local_path: String,
        parse_error: String,
        message: String,
    },
    EventOnly {
        book_id: Uuid,
        local_path: String,
        message: String,
    },
    PageCountOnly {
        book_id: Uuid,
        local_path: String,
        page_count: i32,
        /// Set when the full analysis failed and only the page count could be
        /// recovered; carries the original parse error for traceability.
        recovered_from: Option<String>,
    },
    Thumbnail {
        book_id: Uuid,
        raw_path: String,
        page_count: i32,
    },
}

struct ThumbnailSettings {
    config: ThumbnailConfig,
    concurrency: usize,
}

async fn load_thumbnail_settings(pool: &sqlx::PgPool) -> ThumbnailSettings {
    let fallback = ThumbnailConfig {
        enabled: true,
        format: Some("webp".to_string()),
        width: 300,
        height: 400,
        quality: 80,
        directory: "/data/thumbnails".to_string(),
        timeout_secs: 120,
    };

    let rows =
        sqlx::query(r#"SELECT key, value FROM app_settings WHERE key IN ('thumbnail', 'limits')"#)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    let mut thumb_value: Option<serde_json::Value> = None;
    let mut limits_value: Option<serde_json::Value> = None;
    for row in rows {
        let key: String = row.get("key");
        let value: serde_json::Value = row.get("value");
        match key.as_str() {
            "thumbnail" => thumb_value = Some(value),
            "limits" => limits_value = Some(value),
            _ => {}
        }
    }

    let timeout_secs = limits_value
        .as_ref()
        .and_then(|v| v.get("timeout_seconds"))
        .and_then(|v| v.as_u64())
        .unwrap_or(fallback.timeout_secs);

    // Default: half the logical CPUs, clamped between 2 and 8.
    // Archive extraction is I/O bound but benefits from moderate parallelism.
    let default_concurrency = (num_cpus::get() / 2).clamp(1, 2);
    let concurrency = limits_value
        .as_ref()
        .and_then(|v| v.get("concurrent_renders"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(default_concurrency);

    let config = match thumb_value {
        Some(value) => ThumbnailConfig {
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
        },
        None => ThumbnailConfig {
            timeout_secs,
            ..fallback
        },
    };

    ThumbnailSettings {
        config,
        concurrency,
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
fn fast_jpeg_decode(
    image_bytes: &[u8],
    target_w: u32,
    target_h: u32,
) -> Option<(image::DynamicImage, u32, u32)> {
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
    let (img, orig_w, orig_h) =
        if let Some(result) = fast_jpeg_decode(image_bytes, config.width, config.height) {
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
            let source_format =
                image::guess_format(image_bytes).unwrap_or(image::ImageFormat::Jpeg);
            match source_format {
                image::ImageFormat::Png => {
                    let rgba = resized.to_rgba8();
                    let mut buf = Vec::new();
                    let encoder = image::codecs::png::PngEncoder::new(&mut buf);
                    encoder
                        .write_image(&rgba, w, h, image::ColorType::Rgba8.into())
                        .map_err(|e| anyhow::anyhow!("png encode failed: {}", e))?;
                    Ok(buf)
                }
                _ => {
                    let rgb = resized.to_rgb8();
                    let mut buf = Vec::new();
                    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                        &mut buf,
                        config.quality,
                    );
                    encoder
                        .encode(&rgb, w, h, image::ColorType::Rgb8.into())
                        .map_err(|e| anyhow::anyhow!("jpeg encode failed: {}", e))?;
                    Ok(buf)
                }
            }
        }
        "jpeg" | "jpg" => {
            let rgb = resized.to_rgb8();
            let mut buf = Vec::new();
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, config.quality);
            encoder
                .encode(&rgb, w, h, image::ColorType::Rgb8.into())
                .map_err(|e| anyhow::anyhow!("jpeg encode failed: {}", e))?;
            Ok(buf)
        }
        "png" => {
            let rgba = resized.to_rgba8();
            let mut buf = Vec::new();
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            encoder
                .write_image(&rgba, w, h, image::ColorType::Rgba8.into())
                .map_err(|e| anyhow::anyhow!("png encode failed: {}", e))?;
            Ok(buf)
        }
        _ => {
            // WebP (default)
            let rgb = resized.to_rgb8();
            let rgb_data: &[u8] = rgb.as_raw();
            let quality = config.quality as f32;
            let webp_data =
                webp::Encoder::new(rgb_data, webp::PixelLayout::Rgb, w, h).encode(quality);
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

fn book_format_from_str(s: &str) -> Option<BookFormat> {
    match s {
        "cbz" => Some(BookFormat::Cbz),
        "cbr" => Some(BookFormat::Cbr),
        "pdf" => Some(BookFormat::Pdf),
        "epub" => Some(BookFormat::Epub),
        _ => None,
    }
}

/// Best-effort recovery of a book's page count when `analyze_book` failed
/// (unrenderable first page, corrupt page, timeout, ...).
///
/// The page count does not depend on the first-page render, so recovering it
/// keeps the book visible with a correct page count instead of leaving it
/// permanently stuck on `page_count = NULL`. The thumbnail is left unset: a
/// later thumbnail job retries it. Returns `None` when the page count cannot
/// be determined either (the book then keeps its parse error).
async fn recover_page_count(
    path: &Path,
    format: BookFormat,
    timeout_secs: u64,
    file_name: &str,
) -> Option<i32> {
    let path_owned = path.to_path_buf();
    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        tokio::task::spawn_blocking(move || page_count_only(&path_owned, format)),
    )
    .await
    {
        Ok(Ok(Ok(count))) if count > 0 => {
            info!(
                target: "extraction",
                "[EXTRACTION] Recovered page count ({} pages) for {} despite extraction failure",
                count, file_name
            );
            Some(count)
        }
        Ok(Ok(Ok(_))) => None,
        Ok(Ok(Err(e))) => {
            warn!(target: "extraction", "[EXTRACTION] Page count recovery failed for {}: {}", file_name, e);
            None
        }
        Ok(Err(e)) => {
            warn!(target: "extraction", "[EXTRACTION] Page count recovery spawn error for {}: {}", file_name, e);
            None
        }
        Err(_) => {
            warn!(target: "extraction", "[EXTRACTION] Page count recovery timed out for {}", file_name);
            None
        }
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
    let settings = load_thumbnail_settings(&state.pool).await;
    let config = settings.config;
    let concurrency = settings.concurrency;

    if !config.enabled {
        info!("[ANALYZER] Thumbnails disabled, skipping analysis phase");
        return Ok(());
    }

    let query_filter = if thumbnail_only {
        "b.thumbnail_path IS NULL"
    } else {
        "b.page_count IS NULL"
    };

    let count_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM books b
        JOIN book_files bf ON bf.book_id = b.id
        WHERE (b.library_id = $1 OR $1 IS NULL)
          AND {}
        "#,
        query_filter
    );

    let total: i32 = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(count_sql.as_str()))
        .bind(library_id)
        .fetch_one(&state.pool)
        .await? as i32;

    if total == 0 {
        info!("[ANALYZER] No books to analyze");
        return Ok(());
    }

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

    // -------------------------------------------------------------------------
    // Sub-phase A: extract first page from each archive and store raw image
    // Processed in batches of 500 to limit memory — raw_bytes are freed between batches.
    // The collected results (Uuid, String, i32) are lightweight (~100 bytes each).
    // -------------------------------------------------------------------------
    const BATCH_SIZE: usize = 200;
    const FETCH_BATCH: i64 = 500;

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
    let mut attempted: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    // Failed books keep matching the filter; excluding attempted ids guarantees termination.
    let fetch_sql = format!(
        r#"
        SELECT b.id AS book_id, bf.abs_path, bf.format, (b.thumbnail_path IS NULL) AS needs_thumbnail
        FROM books b
        JOIN book_files bf ON bf.book_id = b.id
        WHERE (b.library_id = $1 OR $1 IS NULL)
          AND {}
          AND NOT (b.id = ANY($3))
        LIMIT $2
        "#,
        query_filter
    );

    loop {
        if cancelled_flag.load(Ordering::Relaxed) {
            break;
        }

        let attempted_ids: Vec<Uuid> = attempted.iter().copied().collect();
        let rows = sqlx::query(sqlx::AssertSqlSafe(fetch_sql.as_str()))
            .bind(library_id)
            .bind(FETCH_BATCH)
            .bind(&attempted_ids)
            .fetch_all(&state.pool)
            .await?;

        if rows.is_empty() {
            break;
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

        for task in &tasks {
            attempted.insert(task.book_id);
        }

        let num_batches = tasks.len().div_ceil(BATCH_SIZE);
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
                batch_idx + 1,
                num_batches,
                batch_tasks.len()
            );

            let batch_outcomes: Vec<ExtractOutcome> = stream::iter(batch_tasks)
                .map(|task| {
                    let config = config.clone();
                    let cancelled = cancelled_flag.clone();

                    async move {
                        if cancelled.load(Ordering::Relaxed) {
                            return ExtractOutcome::Skipped;
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
                            return ExtractOutcome::ResourceFork { book_id };
                        }

                        let format = match book_format_from_str(&task.format) {
                            Some(f) => f,
                            None => {
                                warn!("[ANALYZER] Unknown format '{}' for book {}", task.format, book_id);
                                return ExtractOutcome::Skipped;
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

                        // Keep a copy for the page-count recovery path: the original is moved into the blocking task.
                        let recovery_path = path.to_path_buf();
                        let (page_count, raw_bytes) = match analyze_result {
                            Ok(Ok(Ok(result))) => result,
                            Ok(Ok(Err(e))) => {
                                let parse_error = e.to_string();
                                let message = format!(
                                    "Extraction failed: {} (thumbnail generation failed, page count unavailable)",
                                    file_name
                                );
                                warn!(target: "extraction", "[EXTRACTION] Failed: {} — {}", file_name, parse_error);
                                // The page count is independent from the first-page render:
                                // recover it so the book stays visible with its real error.
                                if let Some(page_count) =
                                    recover_page_count(&recovery_path, format, timeout_secs, &file_name).await
                                {
                                    return ExtractOutcome::PageCountOnly {
                                        book_id,
                                        local_path,
                                        page_count,
                                        recovered_from: Some(parse_error),
                                    };
                                }
                                return ExtractOutcome::Error {
                                    book_id,
                                    local_path,
                                    parse_error,
                                    message,
                                };
                            }
                            Ok(Err(e)) => {
                                warn!(target: "extraction", "[EXTRACTION] spawn error: {} — {}", file_name, e);
                                return ExtractOutcome::EventOnly {
                                    book_id,
                                    local_path,
                                    message: format!("Spawn error: {}", e),
                                };
                            }
                            Err(_) => {
                                warn!(target: "extraction", "[EXTRACTION] Timeout ({}s): {}", timeout_secs, file_name);
                                if let Some(page_count) =
                                    recover_page_count(&recovery_path, format, timeout_secs, &file_name).await
                                {
                                    return ExtractOutcome::PageCountOnly {
                                        book_id,
                                        local_path,
                                        page_count,
                                        recovered_from: Some(format!(
                                            "analyze_book timed out after {}s",
                                            timeout_secs
                                        )),
                                    };
                                }
                                return ExtractOutcome::Error {
                                    book_id,
                                    local_path,
                                    parse_error: format!("analyze_book timed out after {}s", timeout_secs),
                                    message: format!("Extraction timed out after {}s", timeout_secs),
                                };
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
                            return ExtractOutcome::PageCountOnly {
                                book_id,
                                local_path,
                                page_count,
                                recovered_from: None,
                            };
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
                                return ExtractOutcome::Skipped;
                            }
                            Err(e) => {
                                warn!("[ANALYZER] spawn_blocking save_raw error for book {}: {}", book_id, e);
                                return ExtractOutcome::Skipped;
                            }
                        };

                        ExtractOutcome::Thumbnail {
                            book_id,
                            raw_path,
                            page_count,
                        }
                    }
                })
                .buffer_unordered(concurrency)
                .collect()
                .await;

            let mut page_count_updates: Vec<(Uuid, i32)> = Vec::new();
            let mut parse_errors: Vec<(Uuid, String)> = Vec::new();
            let mut events: Vec<EventInsert> = Vec::new();
            let mut resource_forks: Vec<Uuid> = Vec::new();

            let batch_outcomes_len = batch_outcomes.len() as i32;

            for outcome in batch_outcomes {
                match outcome {
                    ExtractOutcome::Skipped => {}
                    ExtractOutcome::ResourceFork { book_id } => resource_forks.push(book_id),
                    ExtractOutcome::Error {
                        book_id,
                        local_path,
                        parse_error,
                        message,
                    } => {
                        parse_errors.push((book_id, parse_error));
                        events.push(EventInsert {
                            job_id,
                            event_type: "error".to_string(),
                            level: "error".to_string(),
                            entity_type: Some("book".to_string()),
                            entity_id: Some(book_id),
                            entity_name: Some(local_path),
                            message: Some(message),
                            detail: None,
                        });
                    }
                    ExtractOutcome::EventOnly {
                        book_id,
                        local_path,
                        message,
                    } => {
                        events.push(EventInsert {
                            job_id,
                            event_type: "error".to_string(),
                            level: "error".to_string(),
                            entity_type: Some("book".to_string()),
                            entity_id: Some(book_id),
                            entity_name: Some(local_path),
                            message: Some(message),
                            detail: None,
                        });
                    }
                    ExtractOutcome::PageCountOnly {
                        book_id,
                        local_path,
                        page_count,
                        recovered_from,
                    } => {
                        page_count_updates.push((book_id, page_count));
                        let (message, level, detail) = match &recovered_from {
                            Some(error) => (
                                format!(
                                    "Recovered {} pages, thumbnail skipped — {}",
                                    page_count, error
                                ),
                                "warning".to_string(),
                                serde_json::json!({"page_count": page_count, "recovered": true, "error": error}),
                            ),
                            None => (
                                format!("Extracted {} pages", page_count),
                                "info".to_string(),
                                serde_json::json!({"page_count": page_count}),
                            ),
                        };
                        events.push(EventInsert {
                            job_id,
                            event_type: "pages_extracted".to_string(),
                            level,
                            entity_type: Some("book".to_string()),
                            entity_id: Some(book_id),
                            entity_name: Some(local_path),
                            message: Some(message),
                            detail: Some(detail),
                        });
                    }
                    ExtractOutcome::Thumbnail {
                        book_id,
                        raw_path,
                        page_count,
                    } => {
                        all_extracted.push((book_id, raw_path, page_count));
                    }
                }
            }

            if !resource_forks.is_empty() {
                sqlx::query("DELETE FROM book_files WHERE book_id = ANY($1)")
                    .bind(&resource_forks)
                    .execute(&state.pool)
                    .await?;
                sqlx::query(
                    "DELETE FROM books WHERE id = ANY($1) AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = books.id)",
                )
                .bind(&resource_forks)
                .execute(&state.pool)
                .await?;
            }

            if let Err(e) = flush_page_counts(&state.pool, &mut page_count_updates).await {
                warn!("[ANALYZER] Failed to flush page counts: {e}");
            }
            if let Err(e) = flush_parse_errors(&state.pool, &mut parse_errors).await {
                warn!("[ANALYZER] Failed to flush parse errors: {e}");
            }
            if let Err(e) = flush_events(&state.pool, &mut events).await {
                warn!("[ANALYZER] Failed to flush events: {e}");
            }

            let processed = extracted_count.fetch_add(batch_outcomes_len, Ordering::Relaxed)
                + batch_outcomes_len;
            let percent = (processed as f64 / total as f64 * 50.0) as i32;
            if let Err(e) = sqlx::query(
                "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
            )
            .bind(job_id)
            .bind(processed)
            .bind(percent)
            .execute(&state.pool)
            .await
            {
                warn!("[ANALYZER] Failed to update job progress: {e}");
            }

            info!(
                target: "extraction",
                "[EXTRACTION] Progress: {}/{} books extracted ({}%)",
                processed, total, percent
            );

            // Log RSS to track memory growth between batches
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        info!(
                            "[ANALYZER] Memory after batch {}/{}: {}",
                            batch_idx + 1,
                            num_batches,
                            line.trim()
                        );
                        break;
                    }
                }
            }
        }
    }

    if cancelled_flag.load(Ordering::Relaxed) {
        cancel_handle.abort();
        info!(
            "[ANALYZER] Job {} cancelled during extraction phase",
            job_id
        );
        return Err(anyhow::anyhow!("Job cancelled by user"));
    }

    let extracted_total = all_extracted.len() as i32;
    let phase_a_elapsed = phase_a_start.elapsed();
    info!(
        "[ANALYZER] Sub-phase A complete: {}/{} books extracted in {:.1}s ({:.0} ms/book)",
        extracted_total,
        total,
        phase_a_elapsed.as_secs_f64(),
        if extracted_total > 0 {
            phase_a_elapsed.as_millis() as f64 / extracted_total as f64
        } else {
            0.0
        },
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

    let resize_chunks: Vec<Vec<(Uuid, String, i32)>> = all_extracted
        .chunks(BATCH_SIZE)
        .map(|c| c.to_vec())
        .collect();

    for chunk in resize_chunks {
        if cancelled_flag.load(Ordering::Relaxed) {
            break;
        }

        let results: Vec<(Uuid, i32, String)> = stream::iter(chunk)
            .map(|(book_id, raw_path, page_count)| {
                let config = config.clone();
                let cancelled = cancelled_flag.clone();

                async move {
                    if cancelled.load(Ordering::Relaxed) {
                        return None;
                    }

                    let raw_path_clone = raw_path.clone();
                    let thumb_result = tokio::task::spawn_blocking(move || {
                        resize_raw_to_thumbnail(book_id, &raw_path_clone, &config)
                    })
                    .await;

                    match thumb_result {
                        Ok(Ok(p)) => Some((book_id, page_count, p)),
                        Ok(Err(e)) => {
                            warn!(
                                "[ANALYZER] resize_raw_to_webp failed for book {}: {}",
                                book_id, e
                            );
                            None
                        }
                        Err(e) => {
                            warn!(
                                "[ANALYZER] spawn_blocking resize error for book {}: {}",
                                book_id, e
                            );
                            None
                        }
                    }
                }
            })
            .buffer_unordered(concurrency)
            .filter_map(|x| async move { x })
            .collect()
            .await;

        let mut thumb_updates: Vec<(Uuid, i32, String)> = Vec::new();
        let mut events: Vec<EventInsert> = Vec::new();

        for (book_id, page_count, thumb_path) in results {
            events.push(EventInsert {
                job_id,
                event_type: "thumbnail_generated".to_string(),
                level: "info".to_string(),
                entity_type: Some("book".to_string()),
                entity_id: Some(book_id),
                entity_name: None,
                message: Some(format!("Thumbnail generated: {}", thumb_path)),
                detail: None,
            });
            thumb_updates.push((book_id, page_count, thumb_path));
        }

        if let Err(e) = flush_thumbnails(&state.pool, &mut thumb_updates).await {
            warn!("[ANALYZER] Failed to flush thumbnails: {e}");
        }
        if let Err(e) = flush_events(&state.pool, &mut events).await {
            warn!("[ANALYZER] Failed to flush events: {e}");
        }

        let thumb_updates_len = thumb_updates.len() as i32;
        let processed =
            resize_count.fetch_add(thumb_updates_len, Ordering::Relaxed) + thumb_updates_len;
        let percent = 50 + (processed as f64 / extracted_total as f64 * 50.0) as i32;
        if let Err(e) = sqlx::query(
            "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
        )
        .bind(job_id)
        .bind(processed)
        .bind(percent)
        .execute(&state.pool)
        .await
        {
            warn!("[ANALYZER] Failed to update job progress: {e}");
        }

        info!(
            target: "thumbnail",
            "[THUMBNAIL] Progress: {}/{} thumbnails generated ({}%)",
            processed, extracted_total, percent
        );
    }

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
        if final_count > 0 {
            phase_b_elapsed.as_millis() as f64 / final_count as f64
        } else {
            0.0
        }
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
    let config = load_thumbnail_settings(&state.pool).await.config;

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
                    warn!(
                        "[ANALYZER] Failed to delete thumbnail {}: {}",
                        path.display(),
                        e
                    );
                } else if *ext != "raw" {
                    deleted_count += 1;
                }
            }
        }
    }
    info!(
        "[ANALYZER] Deleted {} thumbnail files for regeneration",
        deleted_count
    );

    sqlx::query(r#"UPDATE books SET thumbnail_path = NULL WHERE (library_id = $1 OR $1 IS NULL)"#)
        .bind(library_id)
        .execute(&state.pool)
        .await?;

    analyze_library_books(state, job_id, library_id, true).await
}

/// Delete orphaned thumbnail files (books deleted in full_rebuild get new UUIDs).
pub async fn cleanup_orphaned_thumbnails(state: &AppState) -> Result<()> {
    let config = load_thumbnail_settings(&state.pool).await.config;

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
                            warn!(
                                "Failed to delete orphaned file {}: {}",
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

    info!(
        "[ANALYZER] Deleted {} orphaned thumbnail files",
        deleted_count
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Fixtures ───────────────────────────────────────────────────────

    fn rgb_image(width: u32, height: u32) -> image::RgbImage {
        image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 251) as u8, 128])
        })
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let img = image::DynamicImage::ImageRgb8(rgb_image(width, height));
        let mut cursor = std::io::Cursor::new(Vec::new());
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        cursor.into_inner()
    }

    fn jpeg_bytes(width: u32, height: u32, quality: u8) -> Vec<u8> {
        let rgb = rgb_image(width, height);
        let mut buf = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
        encoder
            .encode(rgb.as_raw(), width, height, image::ExtendedColorType::Rgb8)
            .unwrap();
        buf
    }

    fn webp_bytes(width: u32, height: u32) -> Vec<u8> {
        let rgb = rgb_image(width, height);
        webp::Encoder::new(rgb.as_raw(), webp::PixelLayout::Rgb, width, height)
            .encode(80.0)
            .to_vec()
    }

    fn config(dir: &str, format: Option<&str>, width: u32, height: u32) -> ThumbnailConfig {
        ThumbnailConfig {
            enabled: true,
            format: format.map(|f| f.to_string()),
            width,
            height,
            quality: 80,
            directory: dir.to_string(),
            timeout_secs: 60,
        }
    }

    // ─── Pure helpers ───────────────────────────────────────────────────

    #[test]
    fn detect_image_ext_recognizes_png_webp_and_falls_back_to_jpg() {
        assert_eq!(detect_image_ext(&png_bytes(2, 2)), "png");
        assert_eq!(detect_image_ext(&webp_bytes(2, 2)), "webp");
        assert_eq!(detect_image_ext(&jpeg_bytes(2, 2, 80)), "jpg");
        // Unknown bytes default to the most common comic page format.
        assert_eq!(detect_image_ext(b"definitely not an image"), "jpg");
    }

    #[test]
    fn book_format_from_str_maps_supported_extensions() {
        assert_eq!(book_format_from_str("cbz"), Some(BookFormat::Cbz));
        assert_eq!(book_format_from_str("cbr"), Some(BookFormat::Cbr));
        assert_eq!(book_format_from_str("pdf"), Some(BookFormat::Pdf));
        assert_eq!(book_format_from_str("epub"), Some(BookFormat::Epub));
        assert_eq!(
            book_format_from_str("CBZ"),
            None,
            "matching is case-sensitive"
        );
        assert_eq!(book_format_from_str("zip"), None);
    }

    #[test]
    fn generate_thumbnail_defaults_to_downscaled_webp() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config(dir.path().to_str().unwrap(), None, 300, 400);

        let out = generate_thumbnail(&png_bytes(600, 800), &cfg).unwrap();

        assert_eq!(image::guess_format(&out).unwrap(), image::ImageFormat::WebP);
        let decoded = image::load_from_memory(&out).unwrap();
        assert_eq!(decoded.dimensions(), (300, 400));
    }

    #[test]
    fn generate_thumbnail_never_upscales() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config(dir.path().to_str().unwrap(), Some("png"), 300, 400);

        let out = generate_thumbnail(&png_bytes(100, 120), &cfg).unwrap();

        assert_eq!(image::guess_format(&out).unwrap(), image::ImageFormat::Png);
        let decoded = image::load_from_memory(&out).unwrap();
        assert_eq!(decoded.dimensions(), (100, 120));
    }

    #[test]
    fn generate_thumbnail_honors_explicit_format() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().to_str().unwrap();

        let jpeg =
            generate_thumbnail(&png_bytes(400, 600), &config(dir, Some("jpeg"), 100, 150)).unwrap();
        assert_eq!(
            image::guess_format(&jpeg).unwrap(),
            image::ImageFormat::Jpeg
        );

        let png =
            generate_thumbnail(&png_bytes(400, 600), &config(dir, Some("png"), 100, 150)).unwrap();
        assert_eq!(image::guess_format(&png).unwrap(), image::ImageFormat::Png);

        // "original" re-encodes in the source format.
        let png_src = generate_thumbnail(
            &png_bytes(400, 600),
            &config(dir, Some("original"), 100, 150),
        )
        .unwrap();
        assert_eq!(
            image::guess_format(&png_src).unwrap(),
            image::ImageFormat::Png
        );

        let jpg_src = generate_thumbnail(
            &jpeg_bytes(400, 600, 80),
            &config(dir, Some("original"), 100, 150),
        )
        .unwrap();
        assert_eq!(
            image::guess_format(&jpg_src).unwrap(),
            image::ImageFormat::Jpeg
        );
    }

    #[test]
    fn fast_jpeg_decode_scales_jpeg_and_rejects_non_jpeg() {
        let (img, orig_w, orig_h) = fast_jpeg_decode(&jpeg_bytes(800, 1000, 80), 300, 400)
            .expect("a valid JPEG must decode");
        assert_eq!((orig_w, orig_h), (800, 1000));

        let (w, h) = img.dimensions();
        assert!(w > 0 && h > 0);
        assert!(w <= 800 && h <= 1000);
        assert!(
            (w, h) != (800, 1000),
            "DCT scaling should shrink a large JPEG"
        );

        // Non-JPEG input bypasses the fast path.
        assert!(fast_jpeg_decode(&png_bytes(10, 10), 300, 400).is_none());
        assert!(fast_jpeg_decode(&webp_bytes(10, 10), 300, 400).is_none());
    }

    #[test]
    fn raw_image_round_trip_writes_thumbnail_and_removes_raw() {
        let dir = tempfile::tempdir().unwrap();
        let dir_str = dir.path().to_str().unwrap();
        let book_id = Uuid::new_v4();

        let raw_path = save_raw_image(book_id, &png_bytes(600, 800), dir_str).unwrap();
        assert!(raw_path.ends_with(&format!("{book_id}.raw")));
        assert!(Path::new(&raw_path).exists());

        let thumb_path =
            resize_raw_to_thumbnail(book_id, &raw_path, &config(dir_str, Some("png"), 150, 200))
                .unwrap();
        assert!(thumb_path.ends_with(&format!("{book_id}.png")));
        assert!(Path::new(&thumb_path).exists(), "thumbnail must be written");
        assert!(!Path::new(&raw_path).exists(), "raw file must be removed");

        let decoded = image::load_from_memory(&std::fs::read(&thumb_path).unwrap()).unwrap();
        assert_eq!(decoded.dimensions(), (150, 200));
    }

    // ─── Integration tests (require PostgreSQL) ─────────────────────────

    fn write_test_cbz(path: &Path, images: usize) -> std::io::Result<()> {
        use std::io::Write;
        let file = std::fs::File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for i in 0..images {
            zip.start_file(format!("{:03}.png", i + 1), opts)
                .map_err(std::io::Error::other)?;
            zip.write_all(&png_bytes(600, 800))?;
        }
        zip.finish().map_err(std::io::Error::other)?;
        Ok(())
    }

    async fn create_library(pool: &sqlx::PgPool) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(format!("lib-{id}"))
            .bind(format!("/tmp/{id}"))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn create_series(pool: &sqlx::PgPool, library_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Gon')")
            .bind(id)
            .bind(library_id)
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn create_book(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        series_id: Uuid,
        abs_path: &Path,
        format: &str,
    ) -> Uuid {
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, kind, title, format) \
             VALUES ($1, $2, $3, 'comic', 'Gon T01', $4)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(series_id)
        .bind(format)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint) \
             VALUES ($1, $2, $3, $4, 1024, NOW(), 'fp')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(format)
        .bind(abs_path.to_string_lossy().to_string())
        .execute(pool)
        .await
        .unwrap();

        book_id
    }

    async fn create_job(pool: &sqlx::PgPool) -> Uuid {
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, NULL, 'scan', 'running')",
        )
        .bind(job_id)
        .execute(pool)
        .await
        .unwrap();
        job_id
    }

    async fn set_thumbnail_dir(pool: &sqlx::PgPool, directory: &str) {
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ('thumbnail', $1) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(serde_json::json!({
            "enabled": true,
            "format": "webp",
            "width": 300,
            "height": 400,
            "quality": 80,
            "directory": directory,
        }))
        .execute(pool)
        .await
        .unwrap();

        // Keep the analysis timeout generous so a loaded CI machine never flakes.
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ('limits', $1) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(serde_json::json!({ "concurrent_renders": 1, "timeout_seconds": 120 }))
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn analyze_library_books_sets_page_count_and_thumbnail(pool: sqlx::PgPool) {
        let books_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let cbz = books_dir.path().join("Gon T01.cbz");
        write_test_cbz(&cbz, 3).unwrap();

        let lib = create_library(&pool).await;
        let series = create_series(&pool, lib).await;
        let book = create_book(&pool, lib, series, &cbz, "cbz").await;
        let job = create_job(&pool).await;
        set_thumbnail_dir(&pool, thumb_dir.path().to_str().unwrap()).await;

        let state = AppState { pool: pool.clone() };
        analyze_library_books(&state, job, Some(lib), false)
            .await
            .expect("analysis must succeed");

        let (page_count, thumbnail_path): (Option<i32>, Option<String>) =
            sqlx::query_as("SELECT page_count, thumbnail_path FROM books WHERE id = $1")
                .bind(book)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(page_count, Some(3), "the CBZ holds three images");

        let thumb_path = thumbnail_path.expect("a thumbnail path must be stored");
        assert!(
            thumb_path.ends_with(&format!("{book}.webp")),
            "unexpected thumbnail path: {thumb_path}"
        );
        let decoded =
            image::load_from_memory(&std::fs::read(&thumb_path).unwrap()).expect("valid image");
        assert!(decoded.width() <= 300 && decoded.height() <= 400);

        let events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM index_job_events WHERE job_id = $1 AND event_type = 'thumbnail_generated'",
        )
        .bind(job)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(events, 1, "a thumbnail_generated event must be recorded");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn analyze_library_books_records_parse_error_for_corrupt_archive(pool: sqlx::PgPool) {
        let books_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let corrupt = books_dir.path().join("Gon T02.cbz");
        std::fs::write(&corrupt, b"this is definitely not a zip archive").unwrap();

        let lib = create_library(&pool).await;
        let series = create_series(&pool, lib).await;
        let book = create_book(&pool, lib, series, &corrupt, "cbz").await;
        let job = create_job(&pool).await;
        set_thumbnail_dir(&pool, thumb_dir.path().to_str().unwrap()).await;

        let state = AppState { pool: pool.clone() };
        analyze_library_books(&state, job, Some(lib), false)
            .await
            .expect("a corrupt book must not fail the whole job");

        let page_count: Option<i32> =
            sqlx::query_scalar("SELECT page_count FROM books WHERE id = $1")
                .bind(book)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(page_count.is_none(), "the page count cannot be recovered");

        let (status, error): (String, Option<String>) = sqlx::query_as(
            "SELECT parse_status, parse_error_opt FROM book_files WHERE book_id = $1",
        )
        .bind(book)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "error");
        assert!(error.is_some(), "the parse error must be persisted");

        let error_events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM index_job_events WHERE job_id = $1 AND level = 'error'",
        )
        .bind(job)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(error_events, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn regenerate_thumbnails_clears_stale_files_and_rebuilds(pool: sqlx::PgPool) {
        let books_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let cbz = books_dir.path().join("Gon T03.cbz");
        write_test_cbz(&cbz, 2).unwrap();

        let lib = create_library(&pool).await;
        let series = create_series(&pool, lib).await;
        let book = create_book(&pool, lib, series, &cbz, "cbz").await;
        let job = create_job(&pool).await;
        set_thumbnail_dir(&pool, thumb_dir.path().to_str().unwrap()).await;

        // Pretend a previous run produced a stale thumbnail.
        let stale = thumb_dir.path().join(format!("{book}.webp"));
        std::fs::write(&stale, b"stale bytes").unwrap();
        sqlx::query("UPDATE books SET page_count = 2, thumbnail_path = $2 WHERE id = $1")
            .bind(book)
            .bind(stale.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .unwrap();

        let state = AppState { pool: pool.clone() };
        regenerate_thumbnails(&state, job, Some(lib))
            .await
            .expect("regeneration must succeed");

        let thumb_path: Option<String> =
            sqlx::query_scalar("SELECT thumbnail_path FROM books WHERE id = $1")
                .bind(book)
                .fetch_one(&pool)
                .await
                .unwrap();
        let thumb_path = thumb_path.expect("a fresh thumbnail must be stored");
        let bytes = std::fs::read(&thumb_path).unwrap();
        assert!(
            image::load_from_memory(&bytes).is_ok(),
            "the stale placeholder must be replaced by a real image"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn cleanup_orphaned_thumbnails_deletes_unknown_files(pool: sqlx::PgPool) {
        let thumb_dir = tempfile::tempdir().unwrap();
        set_thumbnail_dir(&pool, thumb_dir.path().to_str().unwrap()).await;

        let lib = create_library(&pool).await;
        let series = create_series(&pool, lib).await;
        let kept_book = create_book(&pool, lib, series, Path::new("/tmp/kept.cbz"), "cbz").await;

        let kept = thumb_dir.path().join(format!("{kept_book}.webp"));
        std::fs::write(&kept, b"keep me").unwrap();
        let orphan = thumb_dir.path().join(format!("{}.webp", Uuid::new_v4()));
        std::fs::write(&orphan, b"orphan").unwrap();

        // A non-UUID file must be ignored, not deleted.
        let unrelated = thumb_dir.path().join("README.txt");
        std::fs::write(&unrelated, b"notes").unwrap();

        let state = AppState { pool: pool.clone() };
        cleanup_orphaned_thumbnails(&state).await.unwrap();

        assert!(kept.exists(), "a thumbnail of a live book must be kept");
        assert!(!orphan.exists(), "an orphaned thumbnail must be deleted");
        assert!(unrelated.exists(), "unrelated files must not be touched");
    }
}
