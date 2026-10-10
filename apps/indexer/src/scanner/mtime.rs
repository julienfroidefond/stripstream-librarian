use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::batch::EventInsert;
use crate::utils;
use crate::AppState;

use super::archive::{archive_books, archive_empty_series, archive_orphan_series};
use super::{ExistingFile, JobStats, BATCH_SIZE};

/// Determine whether file deletions should be skipped based on safety heuristics.
/// Returns true if deletions should be skipped (e.g., volume not mounted).
///
/// The last clause catches the case where *every* known file vanished at once (a
/// likely unmounted volume). It only fires when the stale files' parent directories
/// are themselves gone: a library whose files were merely replaced/renamed (e.g. a
/// re-download) still has its directory tree on disk, so deletions must proceed.
pub fn should_skip_deletions(
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
pub async fn handle_stale_deletions(
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
pub async fn upsert_directory_mtimes(
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
