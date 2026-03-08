use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parsers::{detect_format, parse_metadata, BookFormat, ParsedMetadata};
use rayon::prelude::*;
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

pub async fn scan_library(
    state: &AppState,
    job_id: Uuid,
    library_id: Uuid,
    root: &Path,
    stats: &mut JobStats,
    total_processed_count: &mut i32,
    total_files: usize,
    is_full_rebuild: bool,
) -> Result<()> {
    info!("[SCAN] Starting scan of library {} at path: {} (full_rebuild={})", library_id, root.display(), is_full_rebuild);
    
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
        info!("[SCAN] Found {} existing files in database for library {}", existing.len(), library_id);
    } else {
        info!("[SCAN] Full rebuild: skipping existing files lookup (all will be treated as new)");
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

    // Step 1: Collect all book files first
    #[derive(Clone)]
    struct FileInfo {
        path: std::path::PathBuf,
        format: BookFormat,
        abs_path: String,
        file_name: String,
        metadata: std::fs::Metadata,
        mtime: DateTime<Utc>,
        fingerprint: String,
        lookup_path: String,
    }

    let mut file_infos: Vec<FileInfo> = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path().to_path_buf();
        let Some(format) = detect_format(&path) else {
            trace!("[SCAN] Skipping non-book file: {}", path.display());
            continue;
        };

        info!("[SCAN] Found book file: {} (format: {:?})", path.display(), format);
        stats.scanned_files += 1;
        
        let abs_path_local = path.to_string_lossy().to_string();
        let abs_path = utils::unmap_libraries_path(&abs_path_local);
        let file_name = path.file_name()
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

        file_infos.push(FileInfo {
            path,
            format,
            abs_path,
            file_name,
            metadata,
            mtime,
            fingerprint,
            lookup_path,
        });
    }

    info!("[SCAN] Collected {} files, starting parallel parsing", file_infos.len());

    // Step 2: Parse metadata in parallel
    let parsed_results: Vec<(FileInfo, Result<ParsedMetadata>)> = file_infos
        .into_par_iter()
        .map(|file_info| {
            let parse_result = parse_metadata(&file_info.path, file_info.format, root);
            (file_info, parse_result)
        })
        .collect();

    info!("[SCAN] Completed parallel parsing, processing {} results", parsed_results.len());

    // Step 3: Process results sequentially for batch inserts
    for (file_info, parse_result) in parsed_results {
        library_processed_count += 1;
        *total_processed_count += 1;

        // Update progress in DB every 1 second or every 10 files
        let should_update_progress = last_progress_update.elapsed() > Duration::from_secs(1) || library_processed_count % 10 == 0;
        if should_update_progress {
            let progress_percent = if total_files > 0 {
                ((*total_processed_count as f64 / total_files as f64) * 100.0) as i32
            } else {
                0
            };

            sqlx::query(
                "UPDATE index_jobs SET current_file = $2, processed_files = $3, progress_percent = $4 WHERE id = $1"
            )
            .bind(job_id)
            .bind(&file_info.file_name)
            .bind(*total_processed_count)
            .bind(progress_percent)
            .execute(&state.pool)
            .await
            .map_err(|e| {
                error!("[BDD] Failed to update progress for job {}: {}", job_id, e);
                e
            })?;
            
            last_progress_update = std::time::Instant::now();
            
            // Check if job has been cancelled
            if is_job_cancelled(&state.pool, job_id).await? {
                info!("[JOB] Job {} cancelled by user, stopping...", job_id);
                // Flush any pending batches before exiting
                flush_all_batches(&state.pool, &mut books_to_update, &mut files_to_update, &mut books_to_insert, &mut files_to_insert, &mut errors_to_insert).await?;
                return Err(anyhow::anyhow!("Job cancelled by user"));
            }
        }

        let seen_key = utils::remap_libraries_path(&file_info.abs_path);
        seen.insert(seen_key.clone(), true);

        if let Some((file_id, book_id, old_fingerprint)) = existing.get(&file_info.lookup_path).cloned() {
            if !is_full_rebuild && old_fingerprint == file_info.fingerprint {
                trace!("[PROCESS] Skipping unchanged file: {}", file_info.file_name);
                continue;
            }

            info!("[PROCESS] Updating existing file: {} (full_rebuild={}, fingerprint_match={})", file_info.file_name, is_full_rebuild, old_fingerprint == file_info.fingerprint);

            match parse_result {
                Ok(parsed) => {
                    books_to_update.push(BookUpdate {
                        book_id,
                        title: parsed.title,
                        kind: utils::kind_from_format(file_info.format).to_string(),
                        series: parsed.series,
                        volume: parsed.volume,
                        page_count: parsed.page_count,
                    });

                    files_to_update.push(FileUpdate {
                        file_id,
                        format: file_info.format.as_str().to_string(),
                        size_bytes: file_info.metadata.len() as i64,
                        mtime: file_info.mtime,
                        fingerprint: file_info.fingerprint,
                    });

                    stats.indexed_files += 1;
                }
                Err(err) => {
                    warn!("[PARSER] Failed to parse {}: {}", file_info.file_name, err);
                    stats.errors += 1;
                    
                    files_to_update.push(FileUpdate {
                        file_id,
                        format: file_info.format.as_str().to_string(),
                        size_bytes: file_info.metadata.len() as i64,
                        mtime: file_info.mtime,
                        fingerprint: file_info.fingerprint.clone(),
                    });
                    
                    errors_to_insert.push(ErrorInsert {
                        job_id,
                        file_path: file_info.abs_path.clone(),
                        error_message: err.to_string(),
                    });
                    
                    // Also need to mark file as error - we'll do this separately
                    sqlx::query(
                        "UPDATE book_files SET parse_status = 'error', parse_error_opt = $2 WHERE id = $1"
                    )
                    .bind(file_id)
                    .bind(err.to_string())
                    .execute(&state.pool)
                    .await?;
                }
            }

            // Flush if batch is full
            if books_to_update.len() >= BATCH_SIZE || files_to_update.len() >= BATCH_SIZE {
                flush_all_batches(&state.pool, &mut books_to_update, &mut files_to_update, &mut books_to_insert, &mut files_to_insert, &mut errors_to_insert).await?;
            }
            
            continue;
        }

        // New file (thumbnails generated by API after job handoff)
        info!("[PROCESS] Inserting new file: {}", file_info.file_name);
        let book_id = Uuid::new_v4();

        match parse_result {
            Ok(parsed) => {
                let file_id = Uuid::new_v4();

                books_to_insert.push(BookInsert {
                    book_id,
                    library_id,
                    kind: utils::kind_from_format(file_info.format).to_string(),
                    title: parsed.title,
                    series: parsed.series,
                    volume: parsed.volume,
                    page_count: parsed.page_count,
                    thumbnail_path: None,
                });

                files_to_insert.push(FileInsert {
                    file_id,
                    book_id,
                    format: file_info.format.as_str().to_string(),
                    abs_path: file_info.abs_path.clone(),
                    size_bytes: file_info.metadata.len() as i64,
                    mtime: file_info.mtime,
                    fingerprint: file_info.fingerprint,
                    parse_status: "ok".to_string(),
                    parse_error: None,
                });

                stats.indexed_files += 1;
            }
            Err(err) => {
                warn!("[PARSER] Failed to parse {}: {}", file_info.file_name, err);
                stats.errors += 1;
                let book_id = Uuid::new_v4();
                let file_id = Uuid::new_v4();
                
                books_to_insert.push(BookInsert {
                    book_id,
                    library_id,
                    kind: utils::kind_from_format(file_info.format).to_string(),
                    title: utils::file_display_name(&file_info.path),
                    series: None,
                    volume: None,
                    page_count: None,
                    thumbnail_path: None,
                });

                files_to_insert.push(FileInsert {
                    file_id,
                    book_id,
                    format: file_info.format.as_str().to_string(),
                    abs_path: file_info.abs_path.clone(),
                    size_bytes: file_info.metadata.len() as i64,
                    mtime: file_info.mtime,
                    fingerprint: file_info.fingerprint,
                    parse_status: "error".to_string(),
                    parse_error: Some(err.to_string()),
                });

                errors_to_insert.push(ErrorInsert {
                    job_id,
                    file_path: file_info.abs_path,
                    error_message: err.to_string(),
                });
            }
        }

        // Flush if batch is full
        if books_to_insert.len() >= BATCH_SIZE || files_to_insert.len() >= BATCH_SIZE {
            flush_all_batches(&state.pool, &mut books_to_update, &mut files_to_update, &mut books_to_insert, &mut files_to_insert, &mut errors_to_insert).await?;
        }
    }

    // Final flush of any remaining items
    flush_all_batches(&state.pool, &mut books_to_update, &mut files_to_update, &mut books_to_insert, &mut files_to_insert, &mut errors_to_insert).await?;

    info!("[SCAN] Library {} scan complete: {} files scanned, {} indexed, {} errors", 
          library_id, library_processed_count, stats.indexed_files, stats.errors);

    // Handle deletions
    let mut removed_count = 0usize;
    for (abs_path, (file_id, book_id, _)) in existing {
        if seen.contains_key(&abs_path) {
            continue;
        }
        sqlx::query("DELETE FROM book_files WHERE id = $1")
            .bind(file_id)
            .execute(&state.pool)
            .await?;
        sqlx::query("DELETE FROM books WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = $1)")
            .bind(book_id)
            .execute(&state.pool)
            .await?;
        stats.removed_files += 1;
        removed_count += 1;
    }
    
    if removed_count > 0 {
        info!("[SCAN] Removed {} stale files from database", removed_count);
    }

    Ok(())
}
