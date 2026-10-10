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

use crate::batch::{
    flush_all_batches, BookInsert, BookUpdate, ErrorInsert, EventInsert, FileInsert, FileUpdate,
};
use crate::job::is_job_cancelled;
use crate::utils;
use crate::AppState;

use super::archive::restore_archived_data;
use super::mtime::{handle_stale_deletions, upsert_directory_mtimes};
use super::{ExistingFile, JobStats, BATCH_SIZE};

const NOTIFICATION_ITEMS_LIMIT: usize = 10;

pub fn push_capped_item(items: &mut Vec<String>, value: String) {
    if items.len() < NOTIFICATION_ITEMS_LIMIT {
        items.push(value);
    }
}

/// Look up a series by name in the local cache, or INSERT INTO series ... ON CONFLICT DO NOTHING
/// then SELECT to get the id. Updates the cache on creation.
///
/// Cache keys are lowercased so lookups stay O(1) rather than scanning every entry.
pub async fn get_or_create_series_id(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    name: &str,
    cache: &mut HashMap<String, Uuid>,
) -> Result<Uuid> {
    let cache_key = name.to_lowercase();
    if let Some(&id) = cache.get(&cache_key) {
        return Ok(id);
    }

    // Look for existing series with case-insensitive + accent-insensitive match
    // Also checks original_name to prevent duplicates after user renames
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM series WHERE library_id = $1 \
         AND (norm_text(name) = norm_text($2) \
              OR norm_text(original_name) = norm_text($2))",
    )
    .bind(library_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;

    if let Some(id) = existing {
        cache.insert(cache_key, id);
        return Ok(id);
    }

    // No match — insert new series
    sqlx::query(
        "INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3) ON CONFLICT (library_id, name) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(library_id)
    .bind(name)
    .execute(pool)
    .await?;

    let id: Uuid = sqlx::query_scalar("SELECT id FROM series WHERE library_id = $1 AND name = $2")
        .bind(library_id)
        .bind(name)
        .fetch_one(pool)
        .await?;

    cache.insert(cache_key, id);
    Ok(id)
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
            let remapped_path = utils::remap_libraries_path(&abs_path);
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
                let local_path = utils::remap_libraries_path(&db_path);
                let mtime: DateTime<Utc> = row.get("mtime");
                (local_path, mtime)
            })
            .collect()
    } else {
        HashMap::new()
    };

    // Load existing series for this library: name → id
    let series_rows = sqlx::query("SELECT id, name FROM series WHERE library_id = $1")
        .bind(library_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    let mut series_map: HashMap<String, Uuid> = series_rows
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
    let mut last_progress_update = std::time::Instant::now();

    // Batching buffers
    let mut books_to_update: Vec<BookUpdate> = Vec::with_capacity(BATCH_SIZE);
    let mut files_to_update: Vec<FileUpdate> = Vec::with_capacity(BATCH_SIZE);
    let mut books_to_insert: Vec<BookInsert> = Vec::with_capacity(BATCH_SIZE);
    let mut files_to_insert: Vec<FileInsert> = Vec::with_capacity(BATCH_SIZE);
    let mut errors_to_insert: Vec<ErrorInsert> = Vec::with_capacity(BATCH_SIZE);
    let mut events_to_insert: Vec<EventInsert> = Vec::with_capacity(BATCH_SIZE);

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

        // Check if this file is under a skipped dir
        let under_skipped = skipped_dir_prefixes
            .iter()
            .any(|p| local_path.starts_with(p.as_str()));

        if under_skipped {
            // Dir unchanged — just mark file as seen so it's not deleted
            let abs_path_local = local_path.clone();
            let abs_path = utils::unmap_libraries_path(&abs_path_local);
            let lookup_path = utils::remap_libraries_path(&abs_path);
            seen.insert(lookup_path.clone(), true);

            if let Some(existing_file) = existing.get(&lookup_path) {
                let file_id = existing_file.file_id;
                let book_id = existing_file.book_id;
                let old_fingerprint = existing_file.fingerprint.clone();
                let Some(format) = detect_format(&path) else {
                    continue;
                };
                let mut parsed = parse_metadata_fast(&path, format, root);
                // Apply series rename mapping (same as normal scan branch)
                if let Some(ref fs_series) = parsed.series {
                    if let Some(renamed) = series_rename_map.get(fs_series) {
                        debug!(
                            "[SCAN] Mapping renamed series (skipped dir): '{}' → '{}'",
                            fs_series, renamed
                        );
                        parsed.series = Some(renamed.clone());
                    }
                }

                // Detect in-place file changes that the directory mtime did not reflect
                // (e.g. `cp` over an existing file does not bump the parent dir mtime on
                // macOS/Linux). Compute the fingerprint and trigger a re-index if it changed.
                if let Ok(metadata) = std::fs::metadata(&path) {
                    let mtime: DateTime<Utc> = metadata
                        .modified()
                        .map(DateTime::<Utc>::from)
                        .unwrap_or_else(|_| Utc::now());
                    if let Ok(fingerprint) =
                        utils::compute_fingerprint(&path, metadata.len(), &mtime)
                    {
                        if fingerprint != old_fingerprint {
                            debug!(
                                target: "scan",
                                "[SCAN] Fingerprint changed in skipped dir for {}: re-indexing",
                                path.display()
                            );
                            let update_series_id = if let Some(ref series_name) = parsed.series {
                                Some(
                                    get_or_create_series_id(
                                        &state.pool,
                                        library_id,
                                        series_name,
                                        &mut series_map,
                                    )
                                    .await?,
                                )
                            } else {
                                None
                            };

                            books_to_update.push(BookUpdate {
                                book_id,
                                title: parsed.title.clone(),
                                kind: utils::kind_from_format(format).to_string(),
                                format: format.as_str().to_string(),
                                series_id: update_series_id,
                                volume: parsed.volume,
                                volume_type: parsed.volume_type.as_str().to_string(),
                                page_count: None,
                                clear_thumbnail: true,
                            });

                            files_to_update.push(FileUpdate {
                                file_id,
                                format: format.as_str().to_string(),
                                size_bytes: metadata.len() as i64,
                                mtime,
                                fingerprint,
                            });

                            events_to_insert.push(EventInsert {
                                job_id,
                                event_type: "book_updated".to_string(),
                                level: "info".to_string(),
                                entity_type: Some("book".to_string()),
                                entity_id: Some(book_id),
                                entity_name: Some(abs_path.clone()),
                                message: Some(format!(
                                    "Book updated (fingerprint changed in skipped dir): {}",
                                    path.display()
                                )),
                                detail: None,
                            });

                            stats.indexed_files += 1;

                            if books_to_update.len() >= BATCH_SIZE
                                || files_to_update.len() >= BATCH_SIZE
                            {
                                flush_all_batches(
                                    &state.pool,
                                    &mut books_to_update,
                                    &mut files_to_update,
                                    &mut books_to_insert,
                                    &mut files_to_insert,
                                    &mut errors_to_insert,
                                    &mut events_to_insert,
                                )
                                .await?;
                            }

                            continue;
                        }
                    }
                }

                // Fingerprint unchanged — still check if title/volume need updating
                // (e.g., file renamed, or volume not extracted on a previous scan)
                let db_title = &existing_file.title;
                let db_volume = existing_file.volume;
                let db_volume_type = &existing_file.volume_type;
                let parsed_vt = parsed.volume_type.as_str();
                if db_title != &parsed.title
                    || db_volume != parsed.volume
                    || db_volume_type != parsed_vt
                {
                    debug!("[SCAN] Title/volume/type mismatch (skipped dir) for {:?}: DB=('{}', {:?}, '{}') vs parsed=('{}', {:?}, '{}'), updating",
                        path.file_name().unwrap_or_default(), db_title, db_volume, db_volume_type, parsed.title, parsed.volume, parsed_vt);
                    let update_series_id = if let Some(ref series_name) = parsed.series {
                        Some(
                            get_or_create_series_id(
                                &state.pool,
                                library_id,
                                series_name,
                                &mut series_map,
                            )
                            .await?,
                        )
                    } else {
                        None
                    };
                    sqlx::query("UPDATE books SET title = $1, volume = $2, volume_type = $3, series_id = COALESCE($4, series_id), updated_at = NOW() WHERE id = $5")
                        .bind(&parsed.title)
                        .bind(parsed.volume)
                        .bind(parsed_vt)
                        .bind(update_series_id)
                        .bind(book_id)
                        .execute(&state.pool)
                        .await?;
                }
            }
            continue;
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
        stats.scanned_files += 1;

        let abs_path_local = path.to_string_lossy().to_string();
        let abs_path = utils::unmap_libraries_path(&abs_path_local);
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
        let fingerprint = utils::compute_fingerprint(&path, metadata.len(), &mtime)?;
        let lookup_path = utils::remap_libraries_path(&abs_path);

        library_processed_count += 1;
        *total_processed_count += 1;

        // Progress update
        let should_update_progress = last_progress_update.elapsed() > Duration::from_secs(1)
            || library_processed_count % 10 == 0;
        if should_update_progress {
            let progress_percent = if total_files > 0 {
                ((*total_processed_count as f64 / total_files as f64) * 100.0).min(100.0) as i32
            } else {
                0
            };

            if let Err(e) = sqlx::query(
                "UPDATE index_jobs SET current_file = $2, processed_files = $3, progress_percent = $4 WHERE id = $1",
            )
            .bind(job_id)
            .bind(&file_name)
            .bind(*total_processed_count)
            .bind(progress_percent)
            .execute(&state.pool)
            .await
            {
                warn!("[SCAN] Failed to update progress for job {}: {}", job_id, e);
            }

            last_progress_update = std::time::Instant::now();

            if is_job_cancelled(&state.pool, job_id).await? {
                info!("[JOB] Job {} cancelled by user, stopping...", job_id);
                flush_all_batches(
                    &state.pool,
                    &mut books_to_update,
                    &mut files_to_update,
                    &mut books_to_insert,
                    &mut files_to_insert,
                    &mut errors_to_insert,
                    &mut events_to_insert,
                )
                .await?;
                return Err(anyhow::anyhow!("Job cancelled by user"));
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
            let file_id = existing_file.file_id;
            let book_id = existing_file.book_id;
            let old_fingerprint = existing_file.fingerprint.clone();
            if !is_full_rebuild && old_fingerprint == fingerprint {
                // Even if fingerprint hasn't changed, check if title/volume need updating
                // (e.g., after a rename, the file was renamed but title in books table is stale,
                // or volume was not extracted on a previous scan)
                let db_title = &existing_file.title;
                let db_volume = existing_file.volume;
                let db_volume_type = &existing_file.volume_type;
                let parsed_vt = parsed.volume_type.as_str();
                if db_title != &parsed.title
                    || db_volume != parsed.volume
                    || db_volume_type != parsed_vt
                {
                    debug!("[SCAN] Title/volume/type mismatch for {}: DB=('{}', {:?}, '{}') vs parsed=('{}', {:?}, '{}'), updating",
                        file_name, db_title, db_volume, db_volume_type, parsed.title, parsed.volume, parsed_vt);
                    let update_series_id = if let Some(ref series_name) = parsed.series {
                        Some(
                            get_or_create_series_id(
                                &state.pool,
                                library_id,
                                series_name,
                                &mut series_map,
                            )
                            .await?,
                        )
                    } else {
                        None
                    };
                    sqlx::query("UPDATE books SET title = $1, volume = $2, volume_type = $3, series_id = COALESCE($4, series_id), updated_at = NOW() WHERE id = $5")
                        .bind(&parsed.title)
                        .bind(parsed.volume)
                        .bind(parsed_vt)
                        .bind(update_series_id)
                        .bind(book_id)
                        .execute(&state.pool)
                        .await?;

                    events_to_insert.push(EventInsert {
                        job_id,
                        event_type: "book_updated".to_string(),
                        level: "info".to_string(),
                        entity_type: Some("book".to_string()),
                        entity_id: Some(book_id),
                        entity_name: Some(abs_path.clone()),
                        message: Some(format!(
                            "Title/volume updated: ('{}', {:?}) → ('{}', {:?})",
                            db_title, db_volume, parsed.title, parsed.volume
                        )),
                        detail: None,
                    });
                }
                continue;
            }

            debug!(
                target: "scan",
                "[SCAN] Updating: {} (fingerprint_changed={})",
                file_name,
                old_fingerprint != fingerprint
            );

            // Resolve series name → series_id
            let update_series_id = if let Some(ref series_name) = parsed.series {
                Some(
                    get_or_create_series_id(&state.pool, library_id, series_name, &mut series_map)
                        .await?,
                )
            } else {
                None
            };

            books_to_update.push(BookUpdate {
                book_id,
                title: parsed.title,
                kind: utils::kind_from_format(format).to_string(),
                format: format.as_str().to_string(),
                series_id: update_series_id,
                volume: parsed.volume,
                volume_type: parsed.volume_type.as_str().to_string(),
                // Reset page_count so analyzer re-processes this book
                page_count: None,
                clear_thumbnail: true,
            });

            files_to_update.push(FileUpdate {
                file_id,
                format: format.as_str().to_string(),
                size_bytes: metadata.len() as i64,
                mtime,
                fingerprint,
            });

            events_to_insert.push(EventInsert {
                job_id,
                event_type: "book_updated".to_string(),
                level: "info".to_string(),
                entity_type: Some("book".to_string()),
                entity_id: Some(book_id),
                entity_name: Some(abs_path.clone()),
                message: Some(format!("Book updated (fingerprint changed): {}", file_name)),
                detail: None,
            });

            stats.indexed_files += 1;

            if books_to_update.len() >= BATCH_SIZE || files_to_update.len() >= BATCH_SIZE {
                flush_all_batches(
                    &state.pool,
                    &mut books_to_update,
                    &mut files_to_update,
                    &mut books_to_insert,
                    &mut files_to_insert,
                    &mut errors_to_insert,
                    &mut events_to_insert,
                )
                .await?;
            }

            continue;
        }

        // New file — insert with page_count = NULL (analyzer fills it in)
        debug!(target: "scan", "[SCAN] Inserting: {}", file_name);
        let book_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();
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

        // Resolve series name → series_id
        let insert_series_id = if let Some(ref series_name) = parsed.series {
            Some(
                get_or_create_series_id(&state.pool, library_id, series_name, &mut series_map)
                    .await?,
            )
        } else {
            None
        };

        books_to_insert.push(BookInsert {
            book_id,
            library_id,
            kind: utils::kind_from_format(format).to_string(),
            format: format.as_str().to_string(),
            title: parsed.title,
            series_id: insert_series_id,
            volume: parsed.volume,
            volume_type: parsed.volume_type.as_str().to_string(),
            page_count: None,
            thumbnail_path: None,
        });

        files_to_insert.push(FileInsert {
            file_id,
            book_id,
            format: format.as_str().to_string(),
            abs_path: abs_path.clone(),
            size_bytes: metadata.len() as i64,
            mtime,
            fingerprint,
            parse_status: "ok".to_string(),
            parse_error: None,
        });

        events_to_insert.push(EventInsert {
            job_id,
            event_type: "book_added".to_string(),
            level: "info".to_string(),
            entity_type: Some("book".to_string()),
            entity_id: Some(book_id),
            entity_name: Some(abs_path.clone()),
            message: Some(format!("New book discovered: {}", file_name)),
            detail: None,
        });

        stats.indexed_files += 1;
        push_capped_item(&mut stats.new_book_titles, new_book_title);

        if books_to_insert.len() >= BATCH_SIZE || files_to_insert.len() >= BATCH_SIZE {
            flush_all_batches(
                &state.pool,
                &mut books_to_update,
                &mut files_to_update,
                &mut books_to_insert,
                &mut files_to_insert,
                &mut errors_to_insert,
                &mut events_to_insert,
            )
            .await?;
        }
    }

    // Flush remaining batches
    flush_all_batches(
        &state.pool,
        &mut books_to_update,
        &mut files_to_update,
        &mut books_to_insert,
        &mut files_to_insert,
        &mut errors_to_insert,
        &mut events_to_insert,
    )
    .await?;

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
