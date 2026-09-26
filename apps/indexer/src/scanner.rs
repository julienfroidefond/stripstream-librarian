use anyhow::Result;
use chrono::{DateTime, Utc};
use parsers::{detect_format, parse_metadata_fast};
use serde::Serialize;
use sqlx::Row;
use std::{collections::HashMap, path::Path, time::Duration};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    batch::{
        flush_all_batches, BookInsert, BookUpdate, ErrorInsert, EventInsert, FileInsert, FileUpdate,
    },
    job::is_job_cancelled,
    utils, AppState,
};
use std::collections::HashSet;

#[derive(Serialize)]
pub struct JobStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub removed_files: usize,
    pub errors: usize,
    pub warnings: usize,
    pub new_series: usize,
    pub new_series_names: Vec<String>,
    pub new_book_titles: Vec<String>,
}

const BATCH_SIZE: usize = 100;
const NOTIFICATION_ITEMS_LIMIT: usize = 10;

/// A file already present in the DB, with the book fields the scan loop compares against.
struct ExistingFile {
    file_id: Uuid,
    book_id: Uuid,
    fingerprint: String,
    title: String,
    volume: Option<i32>,
    volume_type: String,
    series_id: Option<Uuid>,
}

fn push_capped_item(items: &mut Vec<String>, value: String) {
    if items.len() < NOTIFICATION_ITEMS_LIMIT {
        items.push(value);
    }
}

/// Look up a series by name in the local cache, or INSERT INTO series ... ON CONFLICT DO NOTHING
/// then SELECT to get the id. Updates the cache on creation.
///
/// Cache keys are lowercased so lookups stay O(1) rather than scanning every entry.
async fn get_or_create_series_id(
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

/// Archive books + their files + their reading progress before deletion, in set-based statements.
async fn archive_books(pool: &sqlx::PgPool, book_ids: &[Uuid], file_ids: &[Uuid]) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO archived_books (id, library_id, series_id, series_name, kind, format, title,
                                    author, authors, volume, volume_type, language, page_count,
                                    thumbnail_path, locked_fields, summary, isbn, publish_date,
                                    created_at, updated_at)
        SELECT b.id, b.library_id, b.series_id, s.name, b.kind, b.format, b.title,
               b.author, b.authors, b.volume, b.volume_type, b.language, b.page_count,
               b.thumbnail_path, b.locked_fields, b.summary, b.isbn, b.publish_date,
               b.created_at, b.updated_at
        FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE b.id = ANY($1)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(book_ids)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO archived_book_files (id, archived_book_id, format, abs_path, size_bytes, mtime, fingerprint, created_at)
        SELECT id, book_id, format, abs_path, size_bytes, mtime, fingerprint, created_at
        FROM book_files WHERE id = ANY($1)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(file_ids)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO archived_book_reading_progress (archived_book_id, user_id, status, current_page, last_read_at, updated_at)
        SELECT book_id, user_id, status, current_page, last_read_at, updated_at
        FROM book_reading_progress WHERE book_id = ANY($1)
        ON CONFLICT (archived_book_id, user_id) DO NOTHING
        "#,
    )
    .bind(book_ids)
    .execute(pool)
    .await?;

    Ok(())
}

/// Archive series that are about to lose all their books.
async fn archive_empty_series(pool: &sqlx::PgPool, series_ids: &[Uuid]) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO archived_series (id, library_id, name, description, authors, publishers, genres,
                                     start_year, total_volumes, status, locked_fields, original_name,
                                     book_author, book_language, cover_url, created_at, updated_at)
        SELECT id, library_id, name, description, authors, publishers, genres,
               start_year, total_volumes, status, locked_fields, original_name,
               book_author, book_language, cover_url, created_at, updated_at
        FROM series
        WHERE id = ANY($1)
          AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(series_ids)
    .execute(pool)
    .await?;

    // Preserve AniList link before the CASCADE delete wipes anilist_series_links
    sqlx::query(
        r#"
        UPDATE archived_series aseries
        SET anilist_id    = asl.anilist_id,
            anilist_title = asl.anilist_title,
            anilist_url   = asl.anilist_url
        FROM anilist_series_links asl
        WHERE asl.series_id = aseries.id
          AND aseries.id = ANY($1)
          AND asl.anilist_id IS NOT NULL
          AND aseries.anilist_id IS NULL
        "#,
    )
    .bind(series_ids)
    .execute(pool)
    .await?;

    Ok(())
}

/// Archive orphan series (no books, no metadata links, no available downloads).
async fn archive_orphan_series(pool: &sqlx::PgPool, library_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO archived_series (id, library_id, name, description, authors, publishers, genres,
                                     start_year, total_volumes, status, locked_fields, original_name,
                                     book_author, book_language, cover_url, created_at, updated_at)
        SELECT id, library_id, name, description, authors, publishers, genres,
               start_year, total_volumes, status, locked_fields, original_name,
               book_author, book_language, cover_url, created_at, updated_at
        FROM series
        WHERE library_id = $1
          AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id)
          AND NOT EXISTS (SELECT 1 FROM external_metadata_links WHERE series_id = series.id)
          AND NOT EXISTS (SELECT 1 FROM available_downloads WHERE series_id = series.id)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    // Preserve AniList link before the CASCADE delete wipes anilist_series_links
    sqlx::query(
        r#"
        UPDATE archived_series aseries
        SET anilist_id    = asl.anilist_id,
            anilist_title = asl.anilist_title,
            anilist_url   = asl.anilist_url
        FROM anilist_series_links asl
        JOIN series s ON s.id = asl.series_id
        WHERE asl.series_id = aseries.id
          AND s.library_id = $1
          AND asl.anilist_id IS NOT NULL
          AND aseries.anilist_id IS NULL
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// After a scan, restore reading progress and series metadata for re-discovered books/series.
/// Matches archived records by file path (books) or name+library (series).
pub async fn restore_archived_data(pool: &sqlx::PgPool, library_id: Uuid) -> Result<()> {
    // Restore reading progress by exact file path (same file re-discovered)
    let restored_by_path: i64 = sqlx::query_scalar(
        r#"
        WITH restored AS (
            INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
            SELECT b.id, abrp.user_id, abrp.status, abrp.current_page, abrp.last_read_at, abrp.updated_at
            FROM books b
            JOIN book_files bf ON bf.book_id = b.id
            JOIN archived_book_files abf ON abf.abs_path = bf.abs_path
            JOIN archived_book_reading_progress abrp ON abrp.archived_book_id = abf.archived_book_id
            WHERE b.library_id = $1
            ON CONFLICT (book_id, user_id) DO NOTHING
            RETURNING book_id
        )
        SELECT COUNT(*) FROM restored
        "#,
    )
    .bind(library_id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| {
        warn!(
            "[SCAN] Failed to restore reading progress by path for library {}: {}",
            library_id, e
        );
        0
    });

    // Fallback: match by volume for re-downloads whose filename/extension changed
    // (e.g. "Amulet - T2.cbr" replaced by "Amulet - 02.cbz"). A volume can have several
    // archived rows (multiple editions); keep the most recently archived one per user.
    let restored_by_volume: i64 = sqlx::query_scalar(
        r#"
        WITH restored AS (
            INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
            SELECT DISTINCT ON (b.id, abrp.user_id)
                b.id, abrp.user_id, abrp.status, abrp.current_page, abrp.last_read_at, abrp.updated_at
            FROM books b
            JOIN series s ON s.id = b.series_id
            JOIN archived_books ab
              ON ab.library_id = b.library_id
             AND (
                  ab.series_id = b.series_id
                  OR norm_text(ab.series_name) = norm_text(s.name)
                 )
             AND ab.volume = b.volume
             AND ab.volume_type = b.volume_type
             AND ab.kind = b.kind
            JOIN archived_book_reading_progress abrp ON abrp.archived_book_id = ab.id
            WHERE b.library_id = $1
              AND b.volume IS NOT NULL
              AND b.volume_type = 'regular'
            ORDER BY b.id, abrp.user_id, ab.archived_at DESC
            ON CONFLICT (book_id, user_id) DO NOTHING
            RETURNING book_id
        )
        SELECT COUNT(*) FROM restored
        "#,
    )
    .bind(library_id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| {
        warn!(
            "[SCAN] Failed to restore reading progress by volume for library {}: {}",
            library_id, e
        );
        0
    });

    let restored = restored_by_path + restored_by_volume;
    if restored > 0 {
        info!(
            "[SCAN] Restored reading progress for {} books in library {} ({} by path, {} by volume)",
            restored, library_id, restored_by_path, restored_by_volume
        );
    }

    // Restore series metadata for re-created series (only fill empty fields)
    sqlx::query(
        r#"
        UPDATE series s
        SET
            description     = COALESCE(s.description, aseries.description),
            authors         = CASE WHEN s.authors = '{}' THEN aseries.authors ELSE s.authors END,
            publishers      = CASE WHEN s.publishers = '{}' THEN aseries.publishers ELSE s.publishers END,
            genres          = CASE WHEN s.genres = '{}' THEN aseries.genres ELSE s.genres END,
            total_volumes   = COALESCE(s.total_volumes, aseries.total_volumes),
            status          = COALESCE(s.status, aseries.status),
            cover_url       = COALESCE(s.cover_url, aseries.cover_url),
            locked_fields   = CASE WHEN s.locked_fields = '{}' THEN aseries.locked_fields ELSE s.locked_fields END,
            updated_at      = NOW()
        FROM archived_series aseries
        WHERE s.library_id = $1
          AND s.library_id = aseries.library_id
          AND norm_text(s.name) = norm_text(aseries.name)
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    // Clean up archived books whose files are now active again
    sqlx::query(
        r#"
        DELETE FROM archived_books
        WHERE id IN (
            SELECT ab.id FROM archived_books ab
            JOIN archived_book_files abf ON abf.archived_book_id = ab.id
            JOIN book_files bf ON bf.abs_path = abf.abs_path
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Clean up archived books matched by volume, so resetting a book's progress afterwards is
    // not undone by the next scan re-running the volume fallback.
    sqlx::query(
        r#"
        DELETE FROM archived_books ab
        USING books b, series s
        WHERE s.id = b.series_id
          AND ab.library_id = b.library_id
          AND (
               ab.series_id = b.series_id
               OR norm_text(ab.series_name) = norm_text(s.name)
              )
          AND ab.volume = b.volume
          AND ab.volume_type = b.volume_type
          AND ab.kind = b.kind
          AND b.library_id = $1
          AND b.volume IS NOT NULL
          AND b.volume_type = 'regular'
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    // Clean up archived series whose series is now active again
    sqlx::query(
        r#"
        DELETE FROM archived_series aseries
        WHERE library_id = $1
          AND EXISTS (
              SELECT 1 FROM series s
              WHERE s.library_id = aseries.library_id
                AND norm_text(s.name) = norm_text(aseries.name)
          )
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Determine whether file deletions should be skipped based on safety heuristics.
/// Returns true if deletions should be skipped (e.g., volume not mounted).
///
/// The last clause catches the case where *every* known file vanished at once (a
/// likely unmounted volume). It only fires when the stale files' parent directories
/// are themselves gone: a library whose files were merely replaced/renamed (e.g. a
/// re-download) still has its directory tree on disk, so deletions must proceed.
fn should_skip_deletions(
    root_accessible: bool,
    seen_count: usize,
    existing_count: usize,
    stale_count: usize,
    stale_parents_missing: bool,
) -> bool {
    !root_accessible
        || (seen_count == 0 && existing_count > 0)
        || (stale_count > 0 && stale_count == existing_count && stale_parents_missing)
}

/// Handle deletion of stale files (files in DB but no longer on disk).
/// Includes safety checks to prevent mass deletion if volume is unmounted.
async fn handle_stale_deletions(
    state: &AppState,
    job_id: Uuid,
    library_id: Uuid,
    root: &Path,
    existing: &HashMap<String, ExistingFile>,
    seen: &HashMap<String, bool>,
    stats: &mut JobStats,
) -> Result<()> {
    let existing_count = existing.len();
    let seen_count = seen.len();
    let stale_count = existing
        .iter()
        .filter(|(p, _)| !seen.contains_key(p.as_str()))
        .count();

    let root_accessible = root.is_dir() && std::fs::read_dir(root).is_ok();

    // A stale file whose parent directory is gone points at an unmounted/removed volume
    // rather than a replaced file. Only then is mass deletion treated as suspicious.
    let stale_parents_missing = existing
        .iter()
        .filter(|(p, _)| !seen.contains_key(p.as_str()))
        .all(|(p, _)| Path::new(p).parent().is_none_or(|parent| !parent.is_dir()));

    if should_skip_deletions(
        root_accessible,
        seen_count,
        existing_count,
        stale_count,
        stale_parents_missing,
    ) {
        if stale_count > 0 {
            warn!(
                "[SCAN] Skipping deletion of {} stale files for library {} — \
                 root accessible={}, seen={}, existing={}. \
                 Volume may not be mounted correctly.",
                stale_count, library_id, root_accessible, seen_count, existing_count
            );
            stats.warnings += stale_count;
        }
        return Ok(());
    }

    let mut removed_count = 0usize;
    let mut removal_events: Vec<EventInsert> = Vec::new();
    // Track series that lost books so we can clean up newly-empty ones
    let mut affected_series_ids: HashSet<Uuid> = HashSet::new();

    let mut stale_books: Vec<(Uuid, Uuid)> = Vec::new();

    for (abs_path, existing_file) in existing {
        if seen.contains_key(abs_path) {
            continue;
        }
        let file_id = existing_file.file_id;
        let book_id = existing_file.book_id;

        stale_books.push((book_id, file_id));
        stats.removed_files += 1;
        removed_count += 1;

        if let Some(sid) = existing_file.series_id {
            affected_series_ids.insert(sid);
        }

        removal_events.push(EventInsert {
            job_id,
            event_type: "book_removed".to_string(),
            level: "info".to_string(),
            entity_type: Some("book".to_string()),
            entity_id: Some(book_id),
            entity_name: Some(abs_path.clone()),
            message: Some(format!("Stale book removed: {}", abs_path)),
            detail: None,
        });
    }

    for chunk in stale_books.chunks(BATCH_SIZE) {
        let book_ids: Vec<Uuid> = chunk.iter().map(|(b, _)| *b).collect();
        let file_ids: Vec<Uuid> = chunk.iter().map(|(_, f)| *f).collect();

        if let Err(e) = archive_books(&state.pool, &book_ids, &file_ids).await {
            warn!(
                "[SCAN] Failed to archive {} books before deletion: {}",
                chunk.len(),
                e
            );
        }

        sqlx::query("DELETE FROM book_files WHERE id = ANY($1)")
            .bind(&file_ids)
            .execute(&state.pool)
            .await?;
        sqlx::query(
            "DELETE FROM books WHERE id = ANY($1) AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = books.id)",
        )
        .bind(&book_ids)
        .execute(&state.pool)
        .await?;
    }

    if !removal_events.is_empty() {
        if let Err(e) = crate::batch::flush_events(&state.pool, &mut removal_events).await {
            warn!("[SCAN] Failed to flush removal events: {}", e);
        }
    }

    if removed_count > 0 {
        info!("[SCAN] Removed {} stale files from database", removed_count);
    }

    // Clean up series that just lost ALL their books due to stale file deletion.
    // Archive them first so metadata is preserved for restoration.
    if !affected_series_ids.is_empty() {
        let stale_series_ids: Vec<Uuid> = affected_series_ids.into_iter().collect();

        // Archive series about to lose all books
        if let Err(e) = archive_empty_series(&state.pool, &stale_series_ids).await {
            warn!("[SCAN] Failed to archive series before deletion: {}", e);
        }

        let stale_series_result = sqlx::query_scalar::<_, Uuid>(
            "DELETE FROM series WHERE id = ANY($1) \
             AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id) \
             RETURNING id",
        )
        .bind(&stale_series_ids)
        .fetch_all(&state.pool)
        .await?;

        if !stale_series_result.is_empty() {
            info!(
                "[SCAN] Removed {} series that lost all books (directory removed/renamed)",
                stale_series_result.len()
            );
        }
    }

    // Clean up other orphan series: no books, no metadata links, no available downloads
    // (preserves series added from Discovery that have metadata but no files yet)
    if let Err(e) = archive_orphan_series(&state.pool, library_id).await {
        warn!("[SCAN] Failed to archive orphan series: {}", e);
    }

    let orphan_result = sqlx::query_scalar::<_, i32>(
        "DELETE FROM series WHERE library_id = $1 \
         AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id) \
         AND NOT EXISTS (SELECT 1 FROM external_metadata_links WHERE series_id = series.id) \
         AND NOT EXISTS (SELECT 1 FROM available_downloads WHERE series_id = series.id) \
         RETURNING 1",
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await?;

    let orphan_count = orphan_result.len();
    if orphan_count > 0 {
        info!(
            "[SCAN] Removed {} orphan series (no remaining books)",
            orphan_count
        );
    }

    Ok(())
}

/// Save directory modification times to DB for incremental scan optimization.
async fn upsert_directory_mtimes(
    state: &AppState,
    library_id: Uuid,
    new_dir_mtimes: &[(String, DateTime<Utc>)],
) {
    if new_dir_mtimes.is_empty() {
        return;
    }

    let dir_paths_db: Vec<String> = new_dir_mtimes
        .iter()
        .map(|(local, _)| utils::unmap_libraries_path(local))
        .collect();
    let mtimes: Vec<DateTime<Utc>> = new_dir_mtimes.iter().map(|(_, m)| *m).collect();
    let library_ids: Vec<Uuid> = vec![library_id; new_dir_mtimes.len()];

    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO directory_mtimes (library_id, dir_path, mtime)
        SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::timestamptz[])
        AS t(library_id, dir_path, mtime)
        ON CONFLICT (library_id, dir_path) DO UPDATE SET mtime = EXCLUDED.mtime
        "#,
    )
    .bind(&library_ids)
    .bind(&dir_paths_db)
    .bind(&mtimes)
    .execute(&state.pool)
    .await
    {
        warn!("[SCAN] Failed to upsert directory mtimes: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_deletions_when_root_not_accessible() {
        assert!(should_skip_deletions(false, 10, 10, 5, false));
    }

    #[test]
    fn skip_deletions_when_no_files_seen_but_existing() {
        // Volume probably not mounted — saw 0 files but DB has 50
        assert!(should_skip_deletions(true, 0, 50, 50, false));
    }

    #[test]
    fn skip_deletions_when_all_existing_are_stale_and_parents_missing() {
        // Every DB file is stale AND its directory is gone — unmounted volume, skip
        assert!(should_skip_deletions(true, 5, 10, 10, true));
    }

    #[test]
    fn allow_deletions_when_all_existing_are_stale_but_parents_present() {
        // All files replaced/renamed in place (e.g. re-download) — dirs still exist, delete
        assert!(!should_skip_deletions(true, 5, 10, 10, false));
    }

    #[test]
    fn allow_deletions_normal_case() {
        // Some stale files but most are still present — normal
        assert!(!should_skip_deletions(true, 45, 50, 5, false));
    }

    #[test]
    fn allow_deletions_no_stale() {
        assert!(!should_skip_deletions(true, 50, 50, 0, false));
    }

    #[test]
    fn allow_deletions_empty_db() {
        // No existing files in DB — nothing to delete anyway
        assert!(!should_skip_deletions(true, 10, 0, 0, false));
    }

    #[test]
    fn batch_structs_use_series_id() {
        use crate::batch::{BookInsert, BookUpdate};

        let series_id = Uuid::new_v4();
        let book = BookInsert {
            book_id: Uuid::new_v4(),
            library_id: Uuid::new_v4(),
            kind: "comic".to_string(),
            format: "cbz".to_string(),
            title: "Test".to_string(),
            series_id: Some(series_id),
            volume: Some(1),
            volume_type: "regular".to_string(),
            page_count: None,
            thumbnail_path: None,
        };
        assert_eq!(book.series_id, Some(series_id));

        let update = BookUpdate {
            book_id: Uuid::new_v4(),
            title: "Test".to_string(),
            kind: "comic".to_string(),
            format: "cbz".to_string(),
            series_id: None,
            volume: None,
            volume_type: "regular".to_string(),
            page_count: None,
            clear_thumbnail: false,
        };
        assert_eq!(update.series_id, None);
    }

    // ─── Integration tests (require PostgreSQL) ─────────────────────────

    async fn create_test_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(name)
            .bind(format!("/libraries/{name}"))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_new(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache = HashMap::new();
        let id = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        assert_ne!(id, Uuid::nil());
        assert_eq!(cache.len(), 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_cache_hit(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        let id2 = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        assert_eq!(id1, id2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_cache_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        // Different casing should hit cache
        let id2 = get_or_create_series_id(&pool, lib_id, "one piece", &mut cache)
            .await
            .unwrap();
        assert_eq!(id1, id2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_db_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        // Use two separate caches to bypass cache and test DB lookup
        let mut cache1 = HashMap::new();
        let mut cache2 = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "Dragon Ball", &mut cache1)
            .await
            .unwrap();
        let id2 = get_or_create_series_id(&pool, lib_id, "dragon ball", &mut cache2)
            .await
            .unwrap();
        assert_eq!(id1, id2, "DB lookup should be case-insensitive");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_db_accent_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache1 = HashMap::new();
        let mut cache2 = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "Astérix", &mut cache1)
            .await
            .unwrap();
        let id2 = get_or_create_series_id(&pool, lib_id, "Asterix", &mut cache2)
            .await
            .unwrap();
        assert_eq!(id1, id2, "DB lookup should be accent-insensitive");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_finds_by_original_name(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "rename_test").await;
        let mut cache = HashMap::new();

        // Create series then simulate user rename
        let id1 = get_or_create_series_id(&pool, lib_id, "Dragon Ball", &mut cache)
            .await
            .unwrap();
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind("Dragon Ball Z")
            .bind("Dragon Ball")
            .bind(id1)
            .execute(&pool)
            .await
            .unwrap();

        // Clear cache to force DB lookup
        let mut cache2 = HashMap::new();
        let id2 = get_or_create_series_id(&pool, lib_id, "Dragon Ball", &mut cache2)
            .await
            .unwrap();
        assert_eq!(
            id1, id2,
            "lookup by original_name should return the renamed series"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_original_name_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "rename_case").await;
        let mut cache = HashMap::new();

        let id1 = get_or_create_series_id(&pool, lib_id, "LES MYTHICS", &mut cache)
            .await
            .unwrap();
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind("Mythics")
            .bind("LES MYTHICS")
            .bind(id1)
            .execute(&pool)
            .await
            .unwrap();

        let mut cache2 = HashMap::new();
        let id2 = get_or_create_series_id(&pool, lib_id, "les mythics", &mut cache2)
            .await
            .unwrap();
        assert_eq!(id1, id2, "original_name lookup should be case-insensitive");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_updates_title_when_filename_differs(pool: sqlx::PgPool) {
        // 1. Create a library
        let library_id = create_test_library(&pool, "test_update").await;

        // 2. Create a series
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Series")
            .execute(&pool)
            .await
            .unwrap();

        // 3. Insert a book with title="Old Title" and volume=None
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format, series_id) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind("Old Title")
        .bind("comic")
        .bind("cbz")
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        // 4. Insert a book_file with abs_path containing a DIFFERENT filename
        let file_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7)",
        )
        .bind(file_id)
        .bind(book_id)
        .bind("/libraries/test_update/Series/Series - T05.cbz")
        .bind("cbz")
        .bind(1024_i64)
        .bind("fake_fingerprint")
        .bind("ok")
        .execute(&pool)
        .await
        .unwrap();

        // 5. Verify the book still has the old title
        let db_title: String = sqlx::query_scalar("SELECT title FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(db_title, "Old Title");

        // 6. Simulate what the scanner does: parse the filename, compare, and update
        let parsed_title = "Series - T05";
        let parsed_volume = parsers::extract_volume(parsed_title);

        // Confirm mismatch
        assert_ne!(db_title, parsed_title);

        // Update like the scanner does
        sqlx::query("UPDATE books SET title = $1, volume = $2, updated_at = NOW() WHERE id = $3")
            .bind(parsed_title)
            .bind(parsed_volume)
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();

        // 7. Verify the book now has the new title and volume
        let new_title: String = sqlx::query_scalar("SELECT title FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(new_title, "Series - T05");

        let new_volume: Option<i32> = sqlx::query_scalar("SELECT volume FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(new_volume, Some(5));
    }

    /// Helper: create a book + book_file in DB for title/volume mismatch tests.
    async fn create_book_with_file(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        series_id: Uuid,
        title: &str,
        volume: Option<i32>,
        abs_path: &str,
    ) -> (Uuid, Uuid) {
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, volume, kind, format, series_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(title)
        .bind(volume)
        .bind("comic")
        .bind("cbz")
        .bind(series_id)
        .execute(pool)
        .await
        .unwrap();

        let file_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7)",
        )
        .bind(file_id)
        .bind(book_id)
        .bind(abs_path)
        .bind("cbz")
        .bind(1024_i64)
        .bind("fake_fingerprint")
        .bind("ok")
        .execute(pool)
        .await
        .unwrap();

        (book_id, file_id)
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_updates_title_in_skipped_dir(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "skipped_dir_test").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Series")
            .execute(&pool)
            .await
            .unwrap();

        // Book has "Old Name" with no volume
        let (book_id, _file_id) = create_book_with_file(
            &pool,
            library_id,
            series_id,
            "Old Name",
            None,
            "/libraries/skipped_dir_test/Series/Series - T05.cbz",
        )
        .await;

        // Simulate scanner logic: parse the filename and compare
        let parsed_title = "Series - T05";
        let parsed_volume = parsers::extract_volume(parsed_title);

        let row: (String, Option<i32>) =
            sqlx::query_as("SELECT title, volume FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let (db_title, db_volume) = row;

        // Title mismatch triggers update
        assert_ne!(db_title, parsed_title);
        assert!(db_title != parsed_title || db_volume != parsed_volume);

        sqlx::query("UPDATE books SET title = $1, volume = $2, updated_at = NOW() WHERE id = $3")
            .bind(parsed_title)
            .bind(parsed_volume)
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();

        let new_title: String = sqlx::query_scalar("SELECT title FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let new_volume: Option<i32> = sqlx::query_scalar("SELECT volume FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(new_title, "Series - T05");
        assert_eq!(new_volume, Some(5));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_updates_volume_when_title_matches_but_volume_null(pool: sqlx::PgPool) {
        // Key case: title matches the filename stem but volume is NULL in DB.
        // The fix ensures the scanner also checks volume mismatch, not just title.
        let library_id = create_test_library(&pool, "vol_null_test").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Kaiju no8")
            .execute(&pool)
            .await
            .unwrap();

        // Title matches exactly what parse_metadata_fast would produce, but volume is NULL
        let (book_id, _file_id) = create_book_with_file(
            &pool,
            library_id,
            series_id,
            "Kaiju no8 - Tome 1",
            None, // volume is NULL — this is the bug
            "/libraries/vol_null_test/Kaiju no8/Kaiju no8 - Tome 1.cbz",
        )
        .await;

        // Simulate scanner logic
        let parsed_title = "Kaiju no8 - Tome 1";
        let parsed_volume = parsers::extract_volume(parsed_title);
        assert_eq!(parsed_volume, Some(1), "extract_volume should parse Tome 1");

        let row: (String, Option<i32>) =
            sqlx::query_as("SELECT title, volume FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let (db_title, db_volume) = row;

        // Title matches but volume differs (None vs Some(1))
        assert_eq!(db_title, parsed_title);
        assert_ne!(db_volume, parsed_volume);

        // With the fix, the condition `db_title != parsed.title || db_volume != parsed.volume`
        // catches this case and triggers the update
        if db_title != parsed_title || db_volume != parsed_volume {
            sqlx::query(
                "UPDATE books SET title = $1, volume = $2, updated_at = NOW() WHERE id = $3",
            )
            .bind(parsed_title)
            .bind(parsed_volume)
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let new_volume: Option<i32> = sqlx::query_scalar("SELECT volume FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(new_volume, Some(1), "volume should now be set to 1");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_skips_when_title_and_volume_match(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "no_update_test").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Series")
            .execute(&pool)
            .await
            .unwrap();

        // Book already has correct title AND volume
        let (book_id, _file_id) = create_book_with_file(
            &pool,
            library_id,
            series_id,
            "Series - T05",
            Some(5),
            "/libraries/no_update_test/Series/Series - T05.cbz",
        )
        .await;

        // Record the updated_at before the check
        let before_updated_at: DateTime<Utc> =
            sqlx::query_scalar("SELECT updated_at FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        // Simulate scanner logic
        let parsed_title = "Series - T05";
        let parsed_volume = parsers::extract_volume(parsed_title);
        assert_eq!(parsed_volume, Some(5));

        let row: (String, Option<i32>) =
            sqlx::query_as("SELECT title, volume FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let (db_title, db_volume) = row;

        // Both match — no update should happen
        assert_eq!(db_title, parsed_title);
        assert_eq!(db_volume, parsed_volume);

        let needs_update = db_title != parsed_title || db_volume != parsed_volume;
        assert!(
            !needs_update,
            "no update should be needed when title and volume match"
        );

        // Verify updated_at is unchanged
        let after_updated_at: DateTime<Utc> =
            sqlx::query_scalar("SELECT updated_at FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            before_updated_at, after_updated_at,
            "updated_at should not have changed"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn orphan_series_cleaned_up_after_book_deletion(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "orphan_test").await;

        // Create three series
        let series_with_books = Uuid::new_v4();
        let series_empty_no_links = Uuid::new_v4();
        let series_empty_with_metadata = Uuid::new_v4();
        for (id, name) in [
            (series_with_books, "Has Books"),
            (series_empty_no_links, "Empty No Links"),
            (series_empty_with_metadata, "Empty With Metadata"),
        ] {
            sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(library_id)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }

        // First series has a book
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format, series_id) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind("Book 1")
        .bind("comic")
        .bind("cbz")
        .bind(series_with_books)
        .execute(&pool)
        .await
        .unwrap();

        // Third series has a metadata link (added from Discovery)
        sqlx::query(
            "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, external_url) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(library_id)
        .bind(series_empty_with_metadata)
        .bind("senscritique")
        .bind("12345")
        .bind("https://www.senscritique.com/serie/12345")
        .execute(&pool)
        .await
        .unwrap();

        // Delete orphan series (no books, no metadata, no downloads)
        let deleted = sqlx::query_scalar::<_, i32>(
            "DELETE FROM series WHERE library_id = $1 \
             AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id) \
             AND NOT EXISTS (SELECT 1 FROM external_metadata_links WHERE series_id = series.id) \
             AND NOT EXISTS (SELECT 1 FROM available_downloads WHERE series_id = series.id) \
             RETURNING 1",
        )
        .bind(library_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            deleted.len(),
            1,
            "should delete only the truly orphan series"
        );

        // Verify: series_with_books and series_empty_with_metadata still exist
        let remaining: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM series WHERE library_id = $1 ORDER BY name")
                .bind(library_id)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&series_with_books));
        assert!(remaining.contains(&series_empty_with_metadata));
        assert!(!remaining.contains(&series_empty_no_links));
    }

    /// Series that lost ALL books due to stale file deletion should be removed
    /// even if they have metadata links. This simulates a directory rename/delete.
    /// Discovery-created series (never had books deleted) are preserved.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn stale_deletion_removes_series_that_lost_all_books(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "stale_series_test").await;

        // Series A: had books, will lose them (directory deleted) — has metadata link
        let series_deleted_dir = Uuid::new_v4();
        // Series B: discovery series, never had books — has metadata link
        let series_discovery = Uuid::new_v4();
        // Series C: has books, keeps them
        let series_kept = Uuid::new_v4();

        for (id, name) in [
            (series_deleted_dir, "Deleted Dir Series"),
            (series_discovery, "Discovery Series"),
            (series_kept, "Kept Series"),
        ] {
            sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(library_id)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }

        // Both series_deleted_dir and series_discovery have metadata links
        for sid in [series_deleted_dir, series_discovery] {
            sqlx::query(
                "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
                 VALUES ($1, $2, 'senscritique', $3)",
            )
            .bind(library_id).bind(sid).bind(Uuid::new_v4().to_string())
            .execute(&pool).await.unwrap();
        }

        // series_deleted_dir has a book (will be deleted as stale)
        let stale_book_id = Uuid::new_v4();
        sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id) VALUES ($1, $2, 'Book 1', 'comic', 'cbz', $3)")
            .bind(stale_book_id).bind(library_id).bind(series_deleted_dir).execute(&pool).await.unwrap();

        // series_kept also has a book (stays)
        let kept_book_id = Uuid::new_v4();
        sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id) VALUES ($1, $2, 'Book 2', 'comic', 'cbz', $3)")
            .bind(kept_book_id).bind(library_id).bind(series_kept).execute(&pool).await.unwrap();

        // Simulate stale deletion: delete the book from series_deleted_dir
        sqlx::query("DELETE FROM books WHERE id = $1")
            .bind(stale_book_id)
            .execute(&pool)
            .await
            .unwrap();

        // Now the stale series cleanup: series that just lost all books
        let affected_series_ids = vec![series_deleted_dir];
        let deleted_series = sqlx::query_scalar::<_, Uuid>(
            "DELETE FROM series WHERE id = ANY($1) \
             AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id) \
             RETURNING id",
        )
        .bind(&affected_series_ids)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            deleted_series.len(),
            1,
            "series that lost all books should be deleted"
        );
        assert_eq!(deleted_series[0], series_deleted_dir);

        // Verify discovery series is NOT affected (not in affected_series_ids)
        let discovery_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
                .bind(series_discovery)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(discovery_exists, "discovery series should be preserved");

        // Verify kept series still exists
        let kept_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
                .bind(series_kept)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            kept_exists,
            "series with remaining books should be preserved"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn restore_reading_progress_by_volume_after_extension_change(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "restore_vol").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Amulet')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        // Given: a re-downloaded book for volume 2 with a new filename/extension (.cbr -> .cbz)
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, volume, volume_type, kind, format, series_id) \
             VALUES ($1, $2, 'Amulet - 02', 2, 'regular', 'comic', 'cbz', $3)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, '/libraries/restore_vol/Amulet/Amulet - 02.cbz', 'cbz', 1024, NOW(), 'fp_new', 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .execute(&pool)
        .await
        .unwrap();

        // And: the archived old file for the same volume with its reading progress
        let archived_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO archived_books (id, library_id, series_id, series_name, kind, format, title, volume, volume_type) \
             VALUES ($1, $2, $3, 'Amulet', 'comic', 'cbr', 'Amulet - T2', 2, 'regular')",
        )
        .bind(archived_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, username) VALUES ($1, $2)")
            .bind(user_id)
            .bind(format!("u_{}", user_id))
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO archived_book_reading_progress (archived_book_id, user_id, status, current_page) \
             VALUES ($1, $2, 'read', 42)",
        )
        .bind(archived_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        // When: archived data is restored
        restore_archived_data(&pool, library_id).await.unwrap();

        // Then: progress is restored despite the path/extension change
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(status.as_deref(), Some("read"));

        let page: Option<i32> = sqlx::query_scalar(
            "SELECT current_page FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(page, Some(42));

        // And: the matched archived book is cleaned up so the fallback stays idempotent
        let archived_left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM archived_books WHERE id = $1")
                .bind(archived_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(archived_left, 0, "matched archived book must be cleaned up");

        // And: resetting progress (mark unread deletes the row) survives a later re-scan
        sqlx::query("DELETE FROM book_reading_progress WHERE book_id = $1 AND user_id = $2")
            .bind(book_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        restore_archived_data(&pool, library_id).await.unwrap();

        let status_after_reset: Option<String> = sqlx::query_scalar(
            "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(
            status_after_reset, None,
            "reset progress must not be re-injected"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn restore_reading_progress_by_volume_when_series_renamed(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "restore_renamed").await;

        // Given: a series renamed since the book was archived — same id, different name
        let series_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Amulet (nouvelle edition)')",
        )
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, volume, volume_type, kind, format, series_id) \
             VALUES ($1, $2, 'Amulet - 02', 2, 'regular', 'comic', 'cbz', $3)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        // And: the archived row still carries the old series name
        let archived_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO archived_books (id, library_id, series_id, series_name, kind, format, title, volume, volume_type) \
             VALUES ($1, $2, $3, 'Amulet', 'comic', 'cbr', 'Amulet - T2', 2, 'regular')",
        )
        .bind(archived_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, username) VALUES ($1, $2)")
            .bind(user_id)
            .bind(format!("u_{}", user_id))
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO archived_book_reading_progress (archived_book_id, user_id, status, current_page) \
             VALUES ($1, $2, 'read', 42)",
        )
        .bind(archived_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        // When: archived data is restored
        restore_archived_data(&pool, library_id).await.unwrap();

        // Then: progress is restored via series_id despite the name mismatch
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(status.as_deref(), Some("read"));

        let archived_left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM archived_books WHERE id = $1")
                .bind(archived_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(archived_left, 0, "matched archived book must be cleaned up");
    }
}
