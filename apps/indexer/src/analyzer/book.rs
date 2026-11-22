//! What happens to one book: open its archive once, stage the first page, then
//! publish it.
//!
//! The job keeps its two sub-phases (their status, their progress bar and their
//! batch size are visible in the backoffice); what moves here is the per-book
//! work and, above all, its typed outcome. The translation of that outcome into
//! `analysis_status`, events and covers lives in `record`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use parsers::{analyze_book, BookFormat};
use tracing::{debug, warn};
use uuid::Uuid;

use stripstream_core::paths::LibraryRoot;
use stripstream_thumbnails::ThumbnailStore;

use super::extract_page_count;

/// One book selected for analysis.
#[derive(Clone)]
pub(super) struct BookTask {
    pub book_id: Uuid,
    pub abs_path: String,
    pub format: String,
    pub needs_thumbnail: bool,
}

/// What sub-phase A did with one book.
pub(super) enum Extracted {
    /// Claim released without a verdict (cancellation, unsupported format).
    Skipped { book_id: Uuid },
    /// macOS Apple Double sidecar indexed before the scanner filtered them.
    ResourceFork { book_id: Uuid },
    /// No page count could be recovered.
    Failed {
        book_id: Uuid,
        local_path: String,
        /// `None` when the archive was never read (spawn error): the previous
        /// parse error of the file is left untouched.
        parse_error: Option<String>,
        message: String,
    },
    /// Page count known, no staged cover to publish.
    PagesOnly {
        book_id: Uuid,
        local_path: String,
        page_count: i32,
        /// Set when the full analysis failed and only the page count could be
        /// recovered; carries the original parse error for traceability.
        recovered_from: Option<String>,
    },
    /// First page staged on disk, waiting for sub-phase B.
    Staged {
        book_id: Uuid,
        raw_path: String,
        page_count: i32,
    },
}

/// What sub-phase B did with one staged page.
pub(super) enum Published {
    Cover {
        book_id: Uuid,
        page_count: i32,
        thumb_path: String,
    },
    /// Encoding or publication failed: only the page count from sub-phase A is kept.
    PagesOnly {
        book_id: Uuid,
        page_count: i32,
        error: String,
    },
}

/// The per-book pipeline of the analysis job, shared by the two sub-phases.
#[derive(Clone)]
pub(super) struct BookAnalyzer {
    store: ThumbnailStore,
    library_root: LibraryRoot,
    timeout_secs: u64,
    pdf_scale: u32,
    thumbnails_enabled: bool,
}

impl BookAnalyzer {
    pub(super) fn new(
        store: ThumbnailStore,
        library_root: LibraryRoot,
        timeout_secs: u64,
        pdf_scale: u32,
        thumbnails_enabled: bool,
    ) -> Self {
        Self {
            store,
            library_root,
            timeout_secs,
            pdf_scale,
            thumbnails_enabled,
        }
    }

    /// Sub-phase A: open the archive once and stage its first page.
    pub(super) async fn extract(&self, task: BookTask, cancelled: &AtomicBool) -> Extracted {
        if cancelled.load(Ordering::Relaxed) {
            return Extracted::Skipped {
                book_id: task.book_id,
            };
        }

        let local_path = match self.library_root.resolve(&task.abs_path) {
            Ok(path) => path.to_local().to_string(),
            Err(error) => {
                warn!(
                    "[ANALYZER] Invalid library path '{}' for book {}: {error}",
                    task.abs_path, task.book_id
                );
                return Extracted::Failed {
                    book_id: task.book_id,
                    local_path: task.abs_path,
                    parse_error: Some(error.to_string()),
                    message: format!("Invalid library path: {error}"),
                };
            }
        };
        let path = Path::new(&local_path);
        let book_id = task.book_id;
        let needs_thumbnail = task.needs_thumbnail;

        // Remove macOS Apple Double resource fork files (._*) that were indexed before the scanner filter was added
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("._"))
            .unwrap_or(false)
        {
            warn!(
                "[ANALYZER] Removing macOS resource fork from DB: {}",
                local_path
            );
            return Extracted::ResourceFork { book_id };
        }

        let format = match task.format.parse::<BookFormat>() {
            Ok(f) => f,
            Err(_) => {
                warn!(
                    "[ANALYZER] Unknown format '{}' for book {}",
                    task.format, book_id
                );
                return Extracted::Skipped { book_id };
            }
        };

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| local_path.clone());

        // Thumbnails disabled: still analyse the archive so page counts do
        // not stay unknown forever. The book ends up cover_pending and a
        // later thumbnail job can generate the cover once re-enabled.
        if !self.thumbnails_enabled {
            debug!(target: "extraction", "[EXTRACTION] Page count only (thumbnails disabled): {}", file_name);
            return match extract_page_count(path, format, self.timeout_secs, &file_name).await {
                Some(page_count) => Extracted::PagesOnly {
                    book_id,
                    local_path,
                    page_count,
                    recovered_from: None,
                },
                None => Extracted::Failed {
                    book_id,
                    local_path,
                    parse_error: Some("page count extraction failed".to_string()),
                    message: format!("Page count extraction failed: {}", file_name),
                },
            };
        }

        let path_owned = path.to_path_buf();
        let pdf_scale = self.pdf_scale;

        debug!(target: "extraction", "[EXTRACTION] Starting: {} ({})", file_name, task.format);
        let extract_start = std::time::Instant::now();

        let analyze_result = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
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
                    extract_page_count(&recovery_path, format, self.timeout_secs, &file_name).await
                {
                    return Extracted::PagesOnly {
                        book_id,
                        local_path,
                        page_count,
                        recovered_from: Some(parse_error),
                    };
                }
                return Extracted::Failed {
                    book_id,
                    local_path,
                    parse_error: Some(parse_error),
                    message,
                };
            }
            Ok(Err(e)) => {
                warn!(target: "extraction", "[EXTRACTION] spawn error: {} — {}", file_name, e);
                return Extracted::Failed {
                    book_id,
                    local_path,
                    parse_error: None,
                    message: format!("Spawn error: {}", e),
                };
            }
            Err(_) => {
                warn!(target: "extraction", "[EXTRACTION] Timeout ({}s): {}", self.timeout_secs, file_name);
                if let Some(page_count) =
                    extract_page_count(&recovery_path, format, self.timeout_secs, &file_name).await
                {
                    return Extracted::PagesOnly {
                        book_id,
                        local_path,
                        page_count,
                        recovered_from: Some(format!(
                            "analyze_book timed out after {}s",
                            self.timeout_secs
                        )),
                    };
                }
                return Extracted::Failed {
                    book_id,
                    local_path,
                    parse_error: Some(format!(
                        "analyze_book timed out after {}s",
                        self.timeout_secs
                    )),
                    message: format!("Extraction timed out after {}s", self.timeout_secs),
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
            return Extracted::PagesOnly {
                book_id,
                local_path,
                page_count,
                recovered_from: None,
            };
        }

        // Save raw bytes to disk (no resize, no encode) — moves raw_bytes, no clone
        let store = self.store.clone();
        let raw_path =
            match tokio::task::spawn_blocking(move || store.stage_raw(book_id, &raw_bytes)).await {
                Ok(Ok(p)) => p.to_string_lossy().into_owned(),
                Ok(Err(e)) => {
                    warn!("[ANALYZER] stage_raw failed for book {}: {}", book_id, e);
                    return Extracted::PagesOnly {
                        book_id,
                        local_path,
                        page_count,
                        recovered_from: Some(format!("failed to store raw thumbnail: {e}")),
                    };
                }
                Err(e) => {
                    warn!(
                        "[ANALYZER] spawn_blocking save_raw error for book {}: {}",
                        book_id, e
                    );
                    return Extracted::PagesOnly {
                        book_id,
                        local_path,
                        page_count,
                        recovered_from: Some(format!("failed to store raw thumbnail: {e}")),
                    };
                }
            };

        Extracted::Staged {
            book_id,
            raw_path,
            page_count,
        }
    }

    /// Sub-phase B: encode the staged page and publish it as the cover.
    ///
    /// Returns `None` when the job was cancelled before this book: the staged
    /// page is left for the next job, exactly as the previous fan-out did.
    pub(super) async fn publish(
        &self,
        book_id: Uuid,
        raw_path: String,
        page_count: i32,
        cancelled: &AtomicBool,
    ) -> Option<Published> {
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }

        let staged = raw_path.clone();
        let store = self.store.clone();
        let thumb_result =
            tokio::task::spawn_blocking(move || store.publish_raw(book_id, Path::new(&staged)))
                .await;

        let error = match thumb_result {
            Ok(Ok(p)) => {
                return Some(Published::Cover {
                    book_id,
                    page_count,
                    thumb_path: p.to_string_lossy().into_owned(),
                })
            }
            Ok(Err(e)) => {
                warn!("[ANALYZER] publish_raw failed for book {}: {}", book_id, e);
                format!("failed to encode thumbnail: {e}")
            }
            Err(e) => {
                warn!(
                    "[ANALYZER] spawn_blocking resize error for book {}: {}",
                    book_id, e
                );
                format!("failed to encode thumbnail: {e}")
            }
        };
        // The page count from sub-phase A is still valid: keep it (the book
        // becomes cover_pending) and drop the staged page instead of leaving
        // the book `analyzing` with an orphan `.raw`.
        if let Err(e) = tokio::fs::remove_file(&raw_path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!("[ANALYZER] Failed to remove raw page {}: {}", raw_path, e);
            }
        }
        Some(Published::PagesOnly {
            book_id,
            page_count,
            error,
        })
    }
}
