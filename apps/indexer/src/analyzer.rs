use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use parsers::{analyze_book, page_count_only, BookFormat};
use sqlx::Row;
use tracing::{debug, info, warn};
use uuid::Uuid;

use stripstream_core::analysis::AnalysisStatus;
use stripstream_core::settings::load_setting;
use stripstream_thumbnails::{ThumbnailSettings, ThumbnailStore};

use crate::{
    batch::{
        claim_analysis_books, flush_analysis_status, flush_events, flush_page_counts,
        flush_parse_errors, flush_thumbnails, EventInsert,
    },
    job::is_job_cancelled,
    AppState,
};

/// Result of extracting one book, collected per batch so DB writes can be batched.
enum ExtractOutcome {
    /// Claim released without a verdict (cancellation, unsupported format).
    Skipped {
        book_id: Uuid,
    },
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

/// Thumbnail store plus the `limits` knobs the analysis loop needs.
struct AnalysisSettings {
    store: ThumbnailStore,
    timeout_secs: u64,
    concurrency: usize,
}

async fn load_analysis_settings(pool: &sqlx::PgPool) -> AnalysisSettings {
    let store = ThumbnailSettings::load_or_default(pool).await.store();
    let limits = load_setting::<serde_json::Value>(pool, "limits")
        .await
        .ok()
        .flatten();

    let timeout_secs = limits
        .as_ref()
        .and_then(|v| v.get("timeout_seconds"))
        .and_then(|v| v.as_u64())
        .unwrap_or(120);

    // Default: half the logical CPUs, clamped between 1 and 2.
    // Archive extraction is I/O bound but benefits from moderate parallelism.
    let default_concurrency = (num_cpus::get() / 2).clamp(1, 2);
    let concurrency = limits
        .as_ref()
        .and_then(|v| v.get("concurrent_renders"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(default_concurrency);

    AnalysisSettings {
        store,
        timeout_secs,
        concurrency,
    }
}

/// Best-effort page count extraction without touching the first page.
///
/// Used when `analyze_book` failed (unrenderable first page, corrupt page,
/// timeout, ...) and when thumbnails are disabled. The page count does not
/// depend on the first-page render, so extracting it keeps the book analysed
/// instead of leaving it permanently stuck on `page_count = NULL`. The
/// thumbnail is left unset: a later thumbnail job retries it. Returns `None`
/// when the page count cannot be determined either.
async fn extract_page_count(
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
                "[EXTRACTION] Extracted page count ({} pages) for {}",
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
/// `thumbnail_only` = true: claim books in `cover_pending` (page count already known).
/// `thumbnail_only` = false: claim `discovered`, `failed` and leftover `analyzing` books.
pub async fn analyze_library_books(
    state: &AppState,
    job_id: Uuid,
    library_id: Option<Uuid>,
    thumbnail_only: bool,
) -> Result<()> {
    let settings = load_analysis_settings(&state.pool).await;
    let store = settings.store;
    let timeout_secs = settings.timeout_secs;
    let concurrency = settings.concurrency;
    let thumbnails_enabled = store.settings().enabled;
    let pdf_scale = store.settings().width.max(store.settings().height);

    if !thumbnails_enabled && thumbnail_only {
        info!("[ANALYZER] Thumbnails disabled, skipping thumbnail generation");
        return Ok(());
    }

    let query_filter = if thumbnail_only {
        "b.analysis_status = 'cover_pending'"
    } else {
        "b.analysis_status IN ('discovered', 'analyzing', 'failed')"
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
    if let Err(e) = stripstream_jobs::begin_extraction(&state.pool, job_id, total).await {
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

        let task_ids: Vec<Uuid> = tasks.iter().map(|task| task.book_id).collect();
        claim_analysis_books(&state.pool, &task_ids).await?;

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

            let library_root = state.library_root.clone();
            let batch_outcomes: Vec<ExtractOutcome> = stream::iter(batch_tasks)
                .map(|task| {
                    let store = store.clone();
                    let cancelled = cancelled_flag.clone();
                    let library_root = library_root.clone();

                    async move {
                        if cancelled.load(Ordering::Relaxed) {
                            return ExtractOutcome::Skipped {
                                book_id: task.book_id,
                            };
                        }

                        let local_path = match library_root.resolve(&task.abs_path) {
                            Ok(path) => path.to_local().to_string(),
                            Err(error) => {
                                warn!(
                                    "[ANALYZER] Invalid library path '{}' for book {}: {error}",
                                    task.abs_path, task.book_id
                                );
                                return ExtractOutcome::Error {
                                    book_id: task.book_id,
                                    local_path: task.abs_path,
                                    parse_error: error.to_string(),
                                    message: format!("Invalid library path: {error}"),
                                };
                            }
                        };
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

                        let format = match task.format.parse::<BookFormat>() {
                            Ok(f) => f,
                            Err(_) => {
                                warn!("[ANALYZER] Unknown format '{}' for book {}", task.format, book_id);
                                return ExtractOutcome::Skipped { book_id };
                            }
                        };

                        let file_name = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| local_path.clone());

                        // Thumbnails disabled: still analyse the archive so page counts do
                        // not stay unknown forever. The book ends up cover_pending and a
                        // later thumbnail job can generate the cover once re-enabled.
                        if !thumbnails_enabled {
                            debug!(target: "extraction", "[EXTRACTION] Page count only (thumbnails disabled): {}", file_name);
                            return match extract_page_count(path, format, timeout_secs, &file_name).await {
                                Some(page_count) => ExtractOutcome::PageCountOnly {
                                    book_id,
                                    local_path,
                                    page_count,
                                    recovered_from: None,
                                },
                                None => ExtractOutcome::Error {
                                    book_id,
                                    local_path,
                                    parse_error: "page count extraction failed".to_string(),
                                    message: format!("Page count extraction failed: {}", file_name),
                                },
                            };
                        }

                        let path_owned = path.to_path_buf();

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
                                    extract_page_count(&recovery_path, format, timeout_secs, &file_name).await
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
                                    extract_page_count(&recovery_path, format, timeout_secs, &file_name).await
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
                        let raw_path = match tokio::task::spawn_blocking(move || {
                            store.stage_raw(book_id, &raw_bytes)
                        })
                        .await
                        {
                            Ok(Ok(p)) => p.to_string_lossy().into_owned(),
                            Ok(Err(e)) => {
                                warn!("[ANALYZER] stage_raw failed for book {}: {}", book_id, e);
                                return ExtractOutcome::PageCountOnly {
                                    book_id,
                                    local_path,
                                    page_count,
                                    recovered_from: Some(format!(
                                        "failed to store raw thumbnail: {e}"
                                    )),
                                };
                            }
                            Err(e) => {
                                warn!("[ANALYZER] spawn_blocking save_raw error for book {}: {}", book_id, e);
                                return ExtractOutcome::PageCountOnly {
                                    book_id,
                                    local_path,
                                    page_count,
                                    recovered_from: Some(format!(
                                        "failed to store raw thumbnail: {e}"
                                    )),
                                };
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
            let mut status_updates: Vec<(Uuid, AnalysisStatus)> = Vec::new();
            let mut events: Vec<EventInsert> = Vec::new();
            let mut resource_forks: Vec<Uuid> = Vec::new();

            let batch_outcomes_len = batch_outcomes.len() as i32;

            for outcome in batch_outcomes {
                match outcome {
                    ExtractOutcome::Skipped { book_id } => {
                        status_updates.push((book_id, AnalysisStatus::Discovered));
                    }
                    ExtractOutcome::ResourceFork { book_id } => resource_forks.push(book_id),
                    ExtractOutcome::Error {
                        book_id,
                        local_path,
                        parse_error,
                        message,
                    } => {
                        parse_errors.push((book_id, parse_error));
                        status_updates.push((book_id, AnalysisStatus::Failed));
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
                        status_updates.push((book_id, AnalysisStatus::Failed));
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
            if let Err(e) = flush_analysis_status(&state.pool, &mut status_updates).await {
                warn!("[ANALYZER] Failed to flush analysis statuses: {e}");
            }
            if let Err(e) = flush_events(&state.pool, &mut events).await {
                warn!("[ANALYZER] Failed to flush events: {e}");
            }

            let processed = extracted_count.fetch_add(batch_outcomes_len, Ordering::Relaxed)
                + batch_outcomes_len;
            let percent = (processed as f64 / total as f64 * 50.0) as i32;
            if let Err(e) =
                stripstream_jobs::set_progress(&state.pool, job_id, processed, percent).await
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
    if let Err(e) =
        stripstream_jobs::begin_thumbnail_generation(&state.pool, job_id, extracted_total).await
    {
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
                let store = store.clone();
                let cancelled = cancelled_flag.clone();

                async move {
                    if cancelled.load(Ordering::Relaxed) {
                        return None;
                    }

                    let thumb_result = tokio::task::spawn_blocking(move || {
                        store.publish_raw(book_id, Path::new(&raw_path))
                    })
                    .await;

                    match thumb_result {
                        Ok(Ok(p)) => Some((book_id, page_count, p.to_string_lossy().into_owned())),
                        Ok(Err(e)) => {
                            warn!("[ANALYZER] publish_raw failed for book {}: {}", book_id, e);
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
        if let Err(e) =
            stripstream_jobs::set_progress(&state.pool, job_id, processed, percent).await
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
    let store = ThumbnailSettings::load_or_default(&state.pool)
        .await
        .store();
    if !store.settings().enabled {
        info!("[ANALYZER] Thumbnails disabled, skipping regeneration");
        return Ok(());
    }

    let book_ids_to_clear: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM books
        WHERE (library_id = $1 OR $1 IS NULL)
          AND analysis_status = 'complete'
          AND page_count IS NOT NULL
          AND thumbnail_path IS NOT NULL
        "#,
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let deleted_count = tokio::task::spawn_blocking(move || {
        book_ids_to_clear
            .iter()
            .map(|book_id| store.purge(*book_id))
            .sum::<usize>()
    })
    .await?;
    info!(
        "[ANALYZER] Deleted {} thumbnail files for regeneration",
        deleted_count
    );

    sqlx::query(
        r#"
        UPDATE books SET
            thumbnail_path = NULL,
            analysis_status = 'cover_pending',
            updated_at = NOW()
        WHERE (library_id = $1 OR $1 IS NULL)
          AND analysis_status = 'complete'
          AND page_count IS NOT NULL
        "#,
    )
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    analyze_library_books(state, job_id, library_id, true).await
}

/// Delete orphaned thumbnail files (books deleted in full_rebuild get new UUIDs).
pub async fn cleanup_orphaned_thumbnails(state: &AppState) -> Result<()> {
    let store = ThumbnailSettings::load_or_default(&state.pool)
        .await
        .store();

    let existing_book_ids: std::collections::HashSet<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM books"#)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let deleted_count =
        tokio::task::spawn_blocking(move || store.purge_orphans(&existing_book_ids)).await?;

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

    async fn disable_thumbnails(pool: &sqlx::PgPool) {
        sqlx::query(
            "UPDATE app_settings \
             SET value = jsonb_set(value, '{enabled}', 'false'::jsonb) \
             WHERE key = 'thumbnail'",
        )
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

        let state = crate::test_state(pool.clone());
        analyze_library_books(&state, job, Some(lib), false)
            .await
            .expect("analysis must succeed");

        let (page_count, thumbnail_path, analysis_status): (Option<i32>, Option<String>, String) =
            sqlx::query_as(
                "SELECT page_count, thumbnail_path, analysis_status FROM books WHERE id = $1",
            )
            .bind(book)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(page_count, Some(3), "the CBZ holds three images");
        assert_eq!(analysis_status, "complete");

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
    async fn analyze_library_books_extracts_page_count_when_thumbnails_are_disabled(
        pool: sqlx::PgPool,
    ) {
        let books_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let cbz = books_dir.path().join("Gon T03.cbz");
        write_test_cbz(&cbz, 4).unwrap();

        let lib = create_library(&pool).await;
        let series = create_series(&pool, lib).await;
        let book = create_book(&pool, lib, series, &cbz, "cbz").await;
        let job = create_job(&pool).await;
        set_thumbnail_dir(&pool, thumb_dir.path().to_str().unwrap()).await;
        disable_thumbnails(&pool).await;

        let state = crate::test_state(pool.clone());
        analyze_library_books(&state, job, Some(lib), false)
            .await
            .expect("page-count analysis must still run");

        let (page_count, thumbnail_path, analysis_status): (Option<i32>, Option<String>, String) =
            sqlx::query_as(
                "SELECT page_count, thumbnail_path, analysis_status FROM books WHERE id = $1",
            )
            .bind(book)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(page_count, Some(4));
        assert_eq!(analysis_status, "cover_pending");
        assert!(thumbnail_path.is_none());
        assert!(
            !thumb_dir.path().join(format!("{book}.webp")).exists(),
            "no thumbnail must be written when thumbnails are disabled"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn regenerate_thumbnails_is_a_noop_when_thumbnails_are_disabled(pool: sqlx::PgPool) {
        let books_dir = tempfile::tempdir().unwrap();
        let thumb_dir = tempfile::tempdir().unwrap();
        let cbz = books_dir.path().join("Gon T04.cbz");
        write_test_cbz(&cbz, 2).unwrap();

        let lib = create_library(&pool).await;
        let series = create_series(&pool, lib).await;
        let book = create_book(&pool, lib, series, &cbz, "cbz").await;
        let job = create_job(&pool).await;
        set_thumbnail_dir(&pool, thumb_dir.path().to_str().unwrap()).await;

        let existing = thumb_dir.path().join(format!("{book}.webp"));
        std::fs::write(&existing, b"existing cover").unwrap();
        sqlx::query(
            "UPDATE books SET page_count = 2, thumbnail_path = $2, analysis_status = 'complete' \
             WHERE id = $1",
        )
        .bind(book)
        .bind(existing.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();
        disable_thumbnails(&pool).await;

        let state = crate::test_state(pool.clone());
        regenerate_thumbnails(&state, job, Some(lib))
            .await
            .expect("disabled regeneration must be a no-op");

        assert!(existing.exists(), "existing cover must not be deleted");
        let (thumbnail_path, analysis_status): (Option<String>, String) =
            sqlx::query_as("SELECT thumbnail_path, analysis_status FROM books WHERE id = $1")
                .bind(book)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            thumbnail_path.as_deref(),
            Some(existing.to_string_lossy().as_ref())
        );
        assert_eq!(analysis_status, "complete");
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

        let state = crate::test_state(pool.clone());
        analyze_library_books(&state, job, Some(lib), false)
            .await
            .expect("a corrupt book must not fail the whole job");

        let (page_count, analysis_status): (Option<i32>, String) =
            sqlx::query_as("SELECT page_count, analysis_status FROM books WHERE id = $1")
                .bind(book)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(page_count.is_none(), "the page count cannot be recovered");
        assert_eq!(analysis_status, "failed");

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
        sqlx::query(
            "UPDATE books SET page_count = 2, thumbnail_path = $2, analysis_status = 'complete' WHERE id = $1",
        )
            .bind(book)
            .bind(stale.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .unwrap();

        let state = crate::test_state(pool.clone());
        regenerate_thumbnails(&state, job, Some(lib))
            .await
            .expect("regeneration must succeed");

        let (thumb_path, analysis_status): (Option<String>, String) =
            sqlx::query_as("SELECT thumbnail_path, analysis_status FROM books WHERE id = $1")
                .bind(book)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(analysis_status, "complete");
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

        let state = crate::test_state(pool.clone());
        cleanup_orphaned_thumbnails(&state).await.unwrap();

        assert!(kept.exists(), "a thumbnail of a live book must be kept");
        assert!(!orphan.exists(), "an orphaned thumbnail must be deleted");
        assert!(unrelated.exists(), "unrelated files must not be touched");
    }
}
