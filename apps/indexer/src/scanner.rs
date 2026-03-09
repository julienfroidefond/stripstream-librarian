use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parsers::{detect_format, parse_metadata_fast};
use serde::Serialize;
use sqlx::Row;
use std::{collections::HashMap, path::Path, time::Duration};
use tracing::{error, info, trace, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    batch::{flush_all_batches, BookInsert, BookUpdate, ErrorInsert, FileInsert, FileUpdate},
    job::is_job_cancelled,
    utils,
    AppState,
};

#[derive(Serialize)]
pub struct JobStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub removed_files: usize,
    pub errors: usize,
}

const BATCH_SIZE: usize = 100;

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

    // Load existing files from DB
    let existing_rows = sqlx::query(
        r#"
        SELECT bf.id AS file_id, bf.book_id, bf.abs_path, bf.fingerprint
        FROM book_files bf
        JOIN books b ON b.id = bf.book_id
        WHERE b.library_id = $1
        "#,
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await?;

    let mut existing: HashMap<String, (Uuid, Uuid, String)> = HashMap::new();
    if !is_full_rebuild {
        for row in existing_rows {
            let abs_path: String = row.get("abs_path");
            let remapped_path = utils::remap_libraries_path(&abs_path);
            existing.insert(
                remapped_path,
                (row.get("file_id"), row.get("book_id"), row.get("fingerprint")),
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
        let rows = sqlx::query(
            "SELECT dir_path, mtime FROM directory_mtimes WHERE library_id = $1",
        )
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

    let mut seen: HashMap<String, bool> = HashMap::new();
    let mut library_processed_count = 0i32;
    let mut last_progress_update = std::time::Instant::now();

    // Batching buffers
    let mut books_to_update: Vec<BookUpdate> = Vec::with_capacity(BATCH_SIZE);
    let mut files_to_update: Vec<FileUpdate> = Vec::with_capacity(BATCH_SIZE);
    let mut books_to_insert: Vec<BookInsert> = Vec::with_capacity(BATCH_SIZE);
    let mut files_to_insert: Vec<FileInsert> = Vec::with_capacity(BATCH_SIZE);
    let mut errors_to_insert: Vec<ErrorInsert> = Vec::with_capacity(BATCH_SIZE);

    // Track discovered directory mtimes for upsert after scan
    let mut new_dir_mtimes: Vec<(String, DateTime<Utc>)> = Vec::new();

    // Prefixes (with trailing "/") of directories whose mtime hasn't changed.
    // Files under these prefixes are added to `seen` but not reprocessed.
    let mut skipped_dir_prefixes: Vec<String> = Vec::new();

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
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

        // Check if this file is under a skipped dir
        let under_skipped = skipped_dir_prefixes
            .iter()
            .any(|p| local_path.starts_with(p.as_str()));

        if under_skipped {
            // Dir unchanged — just mark file as seen so it's not deleted
            let abs_path_local = local_path.clone();
            let abs_path = utils::unmap_libraries_path(&abs_path_local);
            let lookup_path = utils::remap_libraries_path(&abs_path);
            seen.insert(lookup_path, true);
            continue;
        }

        let Some(format) = detect_format(&path) else {
            trace!("[SCAN] Skipping non-book file: {}", path.display());
            continue;
        };

        info!(
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

        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("cannot stat {}", path.display()))?;
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
                ((*total_processed_count as f64 / total_files as f64) * 100.0) as i32
            } else {
                0
            };

            sqlx::query(
                "UPDATE index_jobs SET current_file = $2, processed_files = $3, progress_percent = $4 WHERE id = $1",
            )
            .bind(job_id)
            .bind(&file_name)
            .bind(*total_processed_count)
            .bind(progress_percent)
            .execute(&state.pool)
            .await
            .map_err(|e| {
                error!("[BDD] Failed to update progress for job {}: {}", job_id, e);
                e
            })?;

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
                )
                .await?;
                return Err(anyhow::anyhow!("Job cancelled by user"));
            }
        }

        seen.insert(lookup_path.clone(), true);

        // Fast metadata extraction — no archive I/O
        let parsed = parse_metadata_fast(&path, format, root);

        if let Some((file_id, book_id, old_fingerprint)) =
            existing.get(&lookup_path).cloned()
        {
            if !is_full_rebuild && old_fingerprint == fingerprint {
                trace!("[PROCESS] Skipping unchanged file: {}", file_name);
                continue;
            }

            info!(
                "[PROCESS] Updating existing file: {} (fingerprint_changed={})",
                file_name,
                old_fingerprint != fingerprint
            );

            books_to_update.push(BookUpdate {
                book_id,
                title: parsed.title,
                kind: utils::kind_from_format(format).to_string(),
                series: parsed.series,
                volume: parsed.volume,
                // Reset page_count so analyzer re-processes this book
                page_count: None,
            });

            files_to_update.push(FileUpdate {
                file_id,
                format: format.as_str().to_string(),
                size_bytes: metadata.len() as i64,
                mtime,
                fingerprint,
            });

            // Also clear thumbnail so it gets regenerated
            if let Err(e) = sqlx::query(
                "UPDATE books SET thumbnail_path = NULL WHERE id = $1",
            )
            .bind(book_id)
            .execute(&state.pool)
            .await
            {
                warn!(
                    "[BDD] Failed to clear thumbnail for book {}: {}",
                    book_id, e
                );
            }

            stats.indexed_files += 1;

            if books_to_update.len() >= BATCH_SIZE || files_to_update.len() >= BATCH_SIZE {
                flush_all_batches(
                    &state.pool,
                    &mut books_to_update,
                    &mut files_to_update,
                    &mut books_to_insert,
                    &mut files_to_insert,
                    &mut errors_to_insert,
                )
                .await?;
            }

            continue;
        }

        // New file — insert with page_count = NULL (analyzer fills it in)
        info!("[PROCESS] Inserting new file: {}", file_name);
        let book_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();

        books_to_insert.push(BookInsert {
            book_id,
            library_id,
            kind: utils::kind_from_format(format).to_string(),
            title: parsed.title,
            series: parsed.series,
            volume: parsed.volume,
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

        stats.indexed_files += 1;

        if books_to_insert.len() >= BATCH_SIZE || files_to_insert.len() >= BATCH_SIZE {
            flush_all_batches(
                &state.pool,
                &mut books_to_update,
                &mut files_to_update,
                &mut books_to_insert,
                &mut files_to_insert,
                &mut errors_to_insert,
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

    // Handle deletions
    let mut removed_count = 0usize;
    for (abs_path, (file_id, book_id, _)) in &existing {
        if seen.contains_key(abs_path) {
            continue;
        }
        sqlx::query("DELETE FROM book_files WHERE id = $1")
            .bind(file_id)
            .execute(&state.pool)
            .await?;
        sqlx::query(
            "DELETE FROM books WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = $1)",
        )
        .bind(book_id)
        .execute(&state.pool)
        .await?;
        stats.removed_files += 1;
        removed_count += 1;
    }

    if removed_count > 0 {
        info!(
            "[SCAN] Removed {} stale files from database",
            removed_count
        );
    }

    // Upsert directory mtimes for next incremental scan
    if !new_dir_mtimes.is_empty() {
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

    Ok(())
}
