use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::{error, info};
use uuid::Uuid;

use stripstream_jobs::{
    begin_analysis, begin_thumbnail_job, claim_next, cleanup_stale_jobs as cleanup_owner_jobs,
    finish_success, purge_terminal_jobs, set_total_files, JobOwner,
};

use crate::{analyzer, converter, scanner, AppState};

/// Re-match external_book_metadata with book_id IS NULL by volume number.
async fn rematch_unlinked_books(pool: &PgPool, library_id: Uuid) {
    let result = sqlx::query(
        r#"
        UPDATE external_book_metadata ebm
        SET book_id = matched.book_id
        FROM (
            SELECT DISTINCT ON (ebm2.id)
                ebm2.id AS ebm_id,
                b.id AS book_id
            FROM external_book_metadata ebm2
            JOIN external_metadata_links eml ON eml.id = ebm2.link_id
            JOIN books b ON b.series_id = eml.series_id
                AND b.volume = ebm2.volume_number
            WHERE eml.library_id = $1
              AND ebm2.book_id IS NULL
              AND ebm2.volume_number IS NOT NULL
              AND eml.status = 'approved'
        ) matched
        WHERE ebm.id = matched.ebm_id
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            info!(
                "[METADATA] Re-matched {} unlinked external books for library {}",
                r.rows_affected(),
                library_id
            );
        }
        Err(e) => {
            error!("[METADATA] Failed to rematch unlinked books: {e}");
        }
        _ => {}
    }
}

pub async fn cleanup_stale_jobs(pool: &PgPool) -> Result<()> {
    let report = cleanup_owner_jobs(
        pool,
        JobOwner::Indexer,
        Some(std::time::Duration::from_secs(5 * 60)),
        Some(std::time::Duration::from_secs(30 * 60)),
        "Job interrupted by indexer restart",
        "Job was never picked up (stale pending)",
    )
    .await?;

    if report.active > 0 {
        info!(
            "[CLEANUP] Marked {} stale running job(s) as failed",
            report.active
        );
    }
    if report.pending > 0 {
        info!(
            "[CLEANUP] Marked {} stale pending job(s) as failed",
            report.pending
        );
    }

    // Purge terminal jobs older than the retention window. Child rows
    // (index_job_events, *_results, index_job_errors) cascade on delete.
    // Only terminal statuses are purged so in-flight jobs are never removed.
    let purged = purge_terminal_jobs(pool, 90).await?;
    if purged > 0 {
        info!("[CLEANUP] Purged {purged} job(s) older than 90 days");
    }

    Ok(())
}

pub async fn claim_next_job(pool: &PgPool) -> Result<Option<(Uuid, Option<Uuid>)>> {
    Ok(claim_next(pool, JobOwner::Indexer)
        .await?
        .map(|job| (job.id, job.library_id)))
}

pub async fn fail_job(pool: &PgPool, job_id: Uuid, error_message: &str) -> Result<()> {
    stripstream_jobs::fail_job(pool, job_id, error_message, None).await?;
    Ok(())
}

pub async fn is_job_cancelled(pool: &PgPool, job_id: Uuid) -> Result<bool> {
    Ok(stripstream_jobs::is_job_cancelled(pool, job_id).await)
}

pub async fn process_job(
    state: &AppState,
    job_id: Uuid,
    target_library_id: Option<Uuid>,
) -> Result<()> {
    info!(
        "[JOB] Processing {} library={:?}",
        job_id, target_library_id
    );

    let (job_type, book_id): (String, Option<Uuid>) = {
        let row = sqlx::query("SELECT type, book_id FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await?;
        (row.get("type"), row.get("book_id"))
    };

    // CBR to CBZ conversion
    if job_type == "cbr_to_cbz" {
        let book_id =
            book_id.ok_or_else(|| anyhow::anyhow!("cbr_to_cbz job {} has no book_id", job_id))?;
        converter::convert_book(state, job_id, book_id).await?;
        return Ok(());
    }

    // Thumbnail rebuild: generate thumbnails for books missing them
    if job_type == "thumbnail_rebuild" {
        begin_thumbnail_job(&state.pool, job_id).await?;

        analyzer::analyze_library_books(state, job_id, target_library_id, true).await?;

        finish_success(&state.pool, job_id).await?;

        return Ok(());
    }

    // Thumbnail regenerate: clear all thumbnails then re-generate
    if job_type == "thumbnail_regenerate" {
        begin_thumbnail_job(&state.pool, job_id).await?;

        analyzer::regenerate_thumbnails(state, job_id, target_library_id).await?;

        finish_success(&state.pool, job_id).await?;

        return Ok(());
    }

    let is_full_rebuild = job_type == "full_rebuild";
    let is_rescan = job_type == "rescan";
    info!(
        "[JOB] {} type={} full_rebuild={} rescan={}",
        job_id, job_type, is_full_rebuild, is_rescan
    );

    // Rescan: clear directory mtimes to force re-walking all directories,
    // but keep existing data intact (unlike full_rebuild)
    if is_rescan {
        if let Some(library_id) = target_library_id {
            let _ = sqlx::query("DELETE FROM directory_mtimes WHERE library_id = $1")
                .bind(library_id)
                .execute(&state.pool)
                .await;
            info!(
                "[JOB] Rescan: cleared directory mtimes for library {}",
                library_id
            );
        } else {
            let _ = sqlx::query("DELETE FROM directory_mtimes")
                .execute(&state.pool)
                .await;
            info!("[JOB] Rescan: cleared all directory mtimes");
        }
    }

    // Full rebuild: delete existing data first
    if is_full_rebuild {
        info!("[JOB] Full rebuild: deleting existing data");

        if let Some(library_id) = target_library_id {
            sqlx::query(
                "DELETE FROM book_files WHERE book_id IN (SELECT id FROM books WHERE library_id = $1)",
            )
            .bind(library_id)
            .execute(&state.pool)
            .await?;
            sqlx::query("DELETE FROM books WHERE library_id = $1")
                .bind(library_id)
                .execute(&state.pool)
                .await?;
            info!("[JOB] Deleted existing data for library {}", library_id);
        } else {
            sqlx::query("DELETE FROM book_files")
                .execute(&state.pool)
                .await?;
            sqlx::query("DELETE FROM books")
                .execute(&state.pool)
                .await?;
            info!("[JOB] Deleted all existing data");
        }
    }

    let libraries = if let Some(library_id) = target_library_id {
        sqlx::query("SELECT id, root_path FROM libraries WHERE id = $1 AND enabled = TRUE")
            .bind(library_id)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query("SELECT id, root_path FROM libraries WHERE enabled = TRUE")
            .fetch_all(&state.pool)
            .await?
    };

    // Count total files for progress estimation.
    // For incremental rebuilds, use the DB count (instant) — the filesystem will be walked
    // once during discovery anyway, no need for a second full WalkDir pass.
    // For full rebuilds, the DB is already cleared, so we must walk the filesystem.
    let library_ids: Vec<uuid::Uuid> = libraries.iter().map(|r| r.get("id")).collect();

    let total_files: usize = if !is_full_rebuild && !is_rescan {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM book_files bf JOIN books b ON b.id = bf.book_id WHERE b.library_id = ANY($1)"
        )
        .bind(&library_ids)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
        count as usize
    } else {
        let library_paths: Vec<String> = libraries
            .iter()
            .map(|library| {
                crate::utils::remap_libraries_path(&library.get::<String, _>("root_path"))
            })
            .collect();
        // Count sequentially with limited open fds to avoid ENFILE exhaustion
        library_paths
            .iter()
            .map(|root_path| {
                walkdir::WalkDir::new(root_path)
                    .max_open(20)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry.file_type().is_file()
                            && parsers::detect_format(entry.path()).is_some()
                    })
                    .count()
            })
            .sum()
    };

    info!(
        "[JOB] Found {} libraries, {} total files to index",
        libraries.len(),
        total_files
    );

    set_total_files(&state.pool, job_id, total_files as i32).await?;

    let mut stats = scanner::JobStats {
        scanned_files: 0,
        indexed_files: 0,
        removed_files: 0,
        errors: 0,
        warnings: 0,
        new_series: 0,
        new_series_names: Vec::new(),
        new_book_titles: Vec::new(),
    };

    let mut total_processed_count = 0i32;

    // Phase 1: Discovery
    for library in &libraries {
        let library_id: Uuid = library.get("id");
        let root_path: String = library.get("root_path");
        let root_path = crate::utils::remap_libraries_path(&root_path);
        match scanner::scan_library_discovery(
            state,
            job_id,
            library_id,
            std::path::Path::new(&root_path),
            &mut stats,
            &mut total_processed_count,
            total_files,
            is_full_rebuild,
        )
        .await
        {
            Ok(()) => {}
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("cancelled") || err_str.contains("Cancelled") {
                    return Err(err);
                }
                stats.errors += 1;
                error!(library_id = %library_id, error = %err, "library scan failed");
            }
        }
    }

    // For full rebuild: clean up orphaned thumbnail files (old UUIDs)
    if is_full_rebuild {
        analyzer::cleanup_orphaned_thumbnails(state).await?;
    }

    // Phase 2: Analysis (extract page_count + thumbnails for new/updated books)
    begin_analysis(
        &state.pool,
        job_id,
        serde_json::to_value(&stats)?,
        total_processed_count,
    )
    .await?;

    analyzer::analyze_library_books(state, job_id, target_library_id, false).await?;

    // Re-match external book metadata that couldn't be linked during initial metadata fetch
    // (e.g., metadata was fetched before books were scanned)
    if let Some(lib_id) = target_library_id {
        rematch_unlinked_books(&state.pool, lib_id).await;
    }

    finish_success(&state.pool, job_id).await?;

    Ok(())
}
