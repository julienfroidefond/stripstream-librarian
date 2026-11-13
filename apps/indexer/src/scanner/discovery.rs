use anyhow::Result;
use chrono::{DateTime, Utc};
use parsers::{detect_format, parse_metadata_fast};
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use stripstream_jobs::is_job_cancelled;

use crate::job::JobCancelled;
use crate::utils;
use crate::AppState;

use super::archive::restore_archived_data;
use super::mtime::{handle_stale_deletions, upsert_directory_mtimes};
use super::reconcile::{reconcile, FileStat, Observation};
use super::writer::ScanWriter;
use super::{ExistingFile, JobStats};

const NOTIFICATION_ITEMS_LIMIT: usize = 10;
/// Progress is written (and cancellation checked) at least this often.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(1);
/// ...and every this many files outside unchanged directories.
const PROGRESS_EVERY_FILES: i32 = 10;

pub fn push_capped_item(items: &mut Vec<String>, value: String) {
    if items.len() < NOTIFICATION_ITEMS_LIMIT {
        items.push(value);
    }
}

/// Phase 1 — Discovery: walk filesystem, extract metadata from filenames only (no archive I/O).
/// New books are inserted with page_count = NULL so the analyzer phase can fill them in.
/// Updated books (fingerprint changed) get page_count/thumbnail reset.
#[allow(clippy::too_many_arguments)]
pub async fn scan_library_discovery(
    state: &AppState,
    job_id: Uuid,
    library_id: Uuid,
    root: &Path,
    stats: &mut JobStats,
    total_processed_count: &mut i32,
    total_files: usize,
    is_full_rebuild: bool,
) -> Result<()> {
    info!(
        "[SCAN] Starting discovery scan of library {} at path: {} (full_rebuild={})",
        library_id,
        root.display(),
        is_full_rebuild
    );

    // Carries book fields so unchanged files are compared in memory (avoids a SELECT per file).
    let existing_rows = sqlx::query(
        r#"
        SELECT bf.id AS file_id, bf.book_id, bf.abs_path, bf.fingerprint,
               b.title, b.volume, b.volume_type, b.series_id
        FROM book_files bf
        JOIN books b ON b.id = bf.book_id
        WHERE b.library_id = $1
        "#,
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await?;

    let mut existing: HashMap<String, ExistingFile> = HashMap::new();
    if !is_full_rebuild {
        for row in existing_rows {
            let abs_path: String = row.get("abs_path");
            let remapped_path = state
                .library_root
                .resolve(&abs_path)?
                .to_local()
                .to_string();
            existing.insert(
                remapped_path,
                ExistingFile {
                    file_id: row.get("file_id"),
                    book_id: row.get("book_id"),
                    fingerprint: row.get("fingerprint"),
                    title: row.get("title"),
                    volume: row.get("volume"),
                    volume_type: row.get("volume_type"),
                    series_id: row.get("series_id"),
                },
            );
        }
        info!(
            "[SCAN] Found {} existing files in database for library {}",
            existing.len(),
            library_id
        );
    } else {
        info!("[SCAN] Full rebuild: skipping existing files lookup");
        // Delete stale directory mtime records for full rebuild
        let _ = sqlx::query("DELETE FROM directory_mtimes WHERE library_id = $1")
            .bind(library_id)
            .execute(&state.pool)
            .await;
    }

    // Load stored directory mtimes for incremental skip
    let dir_mtimes: HashMap<String, DateTime<Utc>> = if !is_full_rebuild {
        let rows =
            sqlx::query("SELECT dir_path, mtime FROM directory_mtimes WHERE library_id = $1")
                .bind(library_id)
                .fetch_all(&state.pool)
                .await
                .unwrap_or_default();

        rows.into_iter()
            .map(|row| {
                let db_path: String = row.get("dir_path");
                let local_path = state.library_root.resolve(&db_path)?.to_local().to_string();
                let mtime: DateTime<Utc> = row.get("mtime");
                Ok((local_path, mtime))
            })
            .collect::<anyhow::Result<_>>()?
    } else {
        HashMap::new()
    };

    // Load existing series for this library: name → id
    let series_rows = sqlx::query("SELECT id, name FROM series WHERE library_id = $1")
        .bind(library_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    let series_map: HashMap<String, Uuid> = series_rows
        .into_iter()
        .map(|row| {
            let name: String = row.get("name");
            let id: Uuid = row.get("id");
            (name, id)
        })
        .collect();

    // Track existing series names for new_series counting
    let existing_series: HashSet<String> = series_map.keys().cloned().collect();
    let mut seen_new_series: HashSet<String> = HashSet::new();

    // Load series rename mapping: original filesystem name → current DB name.
    // This prevents the scanner from recreating old series after a user rename.
    let rename_rows = sqlx::query(
        "SELECT original_name, name FROM series WHERE library_id = $1 AND original_name IS NOT NULL",
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    let series_rename_map: HashMap<String, String> = rename_rows
        .into_iter()
        .map(|row| {
            let original: String = row.get("original_name");
            let current: String = row.get("name");
            (original, current)
        })
        .collect();
    if !series_rename_map.is_empty() {
        info!(
            "[SCAN] Loaded {} series rename mapping(s) for library {}",
            series_rename_map.len(),
            library_id
        );
    }

    let mut seen: HashMap<String, bool> = HashMap::new();
    let mut library_processed_count = 0i32;
    // Files outside unchanged directories: only they drive the count-based progress trigger.
    let mut changed_dir_count = 0i32;
    let mut last_progress_update = std::time::Instant::now();

    let mut writer = ScanWriter::new(state.pool.clone(), job_id, library_id, series_map);

    // Track discovered directory mtimes for upsert after scan
    let mut new_dir_mtimes: Vec<(String, DateTime<Utc>)> = Vec::new();

    // Prefixes (with trailing "/") of directories whose mtime hasn't changed.
    // Files under these prefixes are added to `seen` but not reprocessed.
    let mut skipped_dir_prefixes: Vec<String> = Vec::new();

    // Track consecutive IO errors to detect fd exhaustion (ENFILE)
    let mut consecutive_io_errors: usize = 0;
    const MAX_CONSECUTIVE_IO_ERRORS: usize = 10;

    for result in WalkDir::new(root).max_open(20).into_iter() {
        let entry = match result {
            Ok(e) => {
                consecutive_io_errors = 0;
                e
            }
            Err(e) => {
                consecutive_io_errors += 1;
                let is_enfile = e
                    .io_error()
                    .map(|io| io.raw_os_error() == Some(23) || io.raw_os_error() == Some(24))
                    .unwrap_or(false);
                if is_enfile || consecutive_io_errors >= MAX_CONSECUTIVE_IO_ERRORS {
                    error!(
                        "[SCAN] Too many IO errors ({} consecutive) scanning library {} — \
                         fd limit likely exhausted. Aborting scan for this library.",
                        consecutive_io_errors, library_id
                    );
                    stats.warnings += 1;
                    break;
                }
                warn!("[SCAN] walkdir error: {}", e);
                stats.warnings += 1;
                continue;
            }
        };

        let path = entry.path().to_path_buf();
        let local_path = path.to_string_lossy().to_string();

        if entry.file_type().is_dir() {
            if entry.depth() == 0 {
                continue; // skip root itself
            }

            // Check if parent dir is already skipped (propagate skip to subdirs)
            let already_under_skipped = skipped_dir_prefixes
                .iter()
                .any(|p| local_path.starts_with(p.as_str()));

            if let Ok(meta) = entry.metadata() {
                if let Ok(sys_mtime) = meta.modified() {
                    let mtime_utc: DateTime<Utc> = DateTime::from(sys_mtime);

                    // Only record mtimes for non-skipped dirs (to avoid polluting DB)
                    if !already_under_skipped {
                        new_dir_mtimes.push((local_path.clone(), mtime_utc));
                    }

                    // Skip if mtime unchanged (incremental only, not already skipped subtree)
                    if !is_full_rebuild && !already_under_skipped {
                        if let Some(&stored_mtime) = dir_mtimes.get(&local_path) {
                            if mtime_utc <= stored_mtime {
                                trace!("[SCAN] Skipping unchanged dir: {}", local_path);
                                // Add trailing slash so starts_with check is exact per-segment
                                skipped_dir_prefixes.push(format!("{}/", local_path));
                            }
                        }
                    }
                }
            }
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        // Skip macOS Apple Double resource fork files (._*)
        let file_name_raw = entry.file_name().to_string_lossy();
        if file_name_raw.starts_with("._") {
            trace!("[SCAN] Skipping macOS resource fork: {}", path.display());
            continue;
        }

        // Files under an unchanged directory are still reconciled: an in-place copy does not
        // bump the parent mtime, and a filename rule may have changed since the last scan.
        let under_skipped = skipped_dir_prefixes
            .iter()
            .any(|p| local_path.starts_with(p.as_str()));

        let library_path = state.library_root.resolve(&local_path)?;
        let abs_path = library_path.to_db().to_string();
        let lookup_path = library_path.to_local().to_string();

        if under_skipped {
            // Unchanged dir: the file is present even if it cannot be stat'ed below.
            seen.insert(lookup_path.clone(), true);
            if !existing.contains_key(&lookup_path) {
                continue;
            }
        }

        let Some(format) = detect_format(&path) else {
            trace!("[SCAN] Skipping non-book file: {}", path.display());
            continue;
        };

        debug!(
            target: "scan",
            "[SCAN] Found book file: {} (format: {:?})",
            path.display(),
            format
        );
        if !under_skipped {
            stats.scanned_files += 1;
        }

        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| abs_path.clone());

        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                let is_enfile = e.raw_os_error() == Some(23) || e.raw_os_error() == Some(24);
                if is_enfile {
                    consecutive_io_errors += 1;
                }
                if consecutive_io_errors >= MAX_CONSECUTIVE_IO_ERRORS {
                    error!(
                        "[SCAN] fd limit exhausted while stat'ing files in library {}. Aborting.",
                        library_id
                    );
                    break;
                }
                warn!("[SCAN] cannot stat {}, skipping: {}", path.display(), e);
                stats.warnings += 1;
                continue;
            }
        };
        let mtime: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let file = FileStat {
            format,
            size_bytes: metadata.len() as i64,
            mtime,
            fingerprint: utils::compute_fingerprint(&path, metadata.len(), &mtime)?,
        };

        library_processed_count += 1;
        *total_processed_count += 1;
        if !under_skipped {
            changed_dir_count += 1;
        }

        // Progress update + cancellation check. Files of unchanged directories only report on
        // elapsed time: on an incremental scan they are the bulk of the library, and a per-count
        // trigger would cost two queries every 10 untouched files.
        let should_update_progress = last_progress_update.elapsed() > PROGRESS_INTERVAL
            || (!under_skipped && changed_dir_count % PROGRESS_EVERY_FILES == 0);
        if should_update_progress {
            if let Err(e) = stripstream_jobs::update_progress_checked(
                &state.pool,
                job_id,
                *total_processed_count,
                total_files as i32,
                &file_name,
            )
            .await
            {
                warn!("[SCAN] Failed to update progress for job {}: {}", job_id, e);
            }

            last_progress_update = std::time::Instant::now();

            if is_job_cancelled(&state.pool, job_id).await {
                info!("[JOB] Job {} cancelled by user, stopping...", job_id);
                writer.flush().await?;
                return Err(JobCancelled.into());
            }
        }

        seen.insert(lookup_path.clone(), true);

        // Fast metadata extraction — no archive I/O
        let mut parsed = parse_metadata_fast(&path, format, root);

        // Apply series rename mapping: if the filesystem-derived series name
        // was renamed by the user, use the current DB name instead.
        if let Some(ref fs_series) = parsed.series {
            if let Some(renamed) = series_rename_map.get(fs_series) {
                debug!(
                    "[SCAN] Mapping renamed series: '{}' → '{}'",
                    fs_series, renamed
                );
                parsed.series = Some(renamed.clone());
            }
        }

        if let Some(existing_file) = existing.get(&lookup_path) {
            let observation = Observation::of(existing_file, file);
            let reconciliation = reconcile(existing_file, &parsed, observation);
            writer
                .apply(reconciliation, &abs_path, &file_name, stats)
                .await?;
            continue;
        }

        // New file — insert with page_count = NULL (analyzer fills it in)
        debug!(target: "scan", "[SCAN] Inserting: {}", file_name);
        let new_book_title = parsed.title.clone();

        // Track new series
        let series_key = parsed
            .series
            .as_deref()
            .unwrap_or("unclassified")
            .to_string();
        if !existing_series.contains(&series_key) && seen_new_series.insert(series_key) {
            stats.new_series += 1;
            push_capped_item(
                &mut stats.new_series_names,
                parsed
                    .series
                    .clone()
                    .unwrap_or_else(|| "unclassified".to_string()),
            );
        }

        writer
            .insert_new(parsed, file, &abs_path, &file_name, stats)
            .await?;
        push_capped_item(&mut stats.new_book_titles, new_book_title);
    }

    // Flush remaining batches
    writer.flush().await?;

    if !skipped_dir_prefixes.is_empty() {
        info!(
            "[SCAN] Skipped {} unchanged directories",
            skipped_dir_prefixes.len()
        );
    }

    info!(
        "[SCAN] Library {} discovery complete: {} files scanned, {} indexed, {} errors",
        library_id, library_processed_count, stats.indexed_files, stats.errors
    );

    handle_stale_deletions(state, job_id, library_id, root, &existing, &seen, stats).await?;
    upsert_directory_mtimes(state, library_id, &new_dir_mtimes).await;

    if stats.indexed_files > 0 || stats.removed_files > 0 {
        if let Err(e) = restore_archived_data(&state.pool, library_id).await {
            warn!(
                "[SCAN] Failed to restore archived data for library {}: {}",
                library_id, e
            );
        }
    }

    Ok(())
}
