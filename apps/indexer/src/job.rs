use anyhow::Result;
use rayon::prelude::*;
use sqlx::{PgPool, Row};
use tracing::{error, info};
use uuid::Uuid;

use crate::{analyzer, converter, meili, scanner, AppState};

pub async fn cleanup_stale_jobs(pool: &PgPool) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE index_jobs
        SET status = 'failed',
            finished_at = NOW(),
            error_opt = 'Job interrupted by indexer restart'
        WHERE status IN ('running', 'extracting_pages', 'generating_thumbnails')
          AND started_at < NOW() - INTERVAL '5 minutes'
        RETURNING id
        "#,
    )
    .fetch_all(pool)
    .await?;

    if !result.is_empty() {
        let count = result.len();
        let ids: Vec<String> = result
            .iter()
            .map(|row| row.get::<Uuid, _>("id").to_string())
            .collect();
        info!(
            "[CLEANUP] Marked {} stale job(s) as failed: {}",
            count,
            ids.join(", ")
        );
    }

    Ok(())
}

pub async fn claim_next_job(pool: &PgPool) -> Result<Option<(Uuid, Option<Uuid>)>> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query(
        r#"
        SELECT j.id, j.type, j.library_id
        FROM index_jobs j
        WHERE j.status = 'pending'
          AND (
            (j.type IN ('rebuild', 'full_rebuild') AND NOT EXISTS (
                SELECT 1 FROM index_jobs
                WHERE status = 'running'
                  AND type IN ('rebuild', 'full_rebuild')
            ))
            OR
            j.type NOT IN ('rebuild', 'full_rebuild')
          )
        ORDER BY
            CASE j.type
                WHEN 'full_rebuild' THEN 1
                WHEN 'rebuild' THEN 2
                ELSE 3
            END,
            j.created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let id: Uuid = row.get("id");
    let job_type: String = row.get("type");
    let library_id: Option<Uuid> = row.get("library_id");

    if job_type == "rebuild" || job_type == "full_rebuild" {
        let has_running_rebuild: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM index_jobs
                WHERE status = 'running'
                  AND type IN ('rebuild', 'full_rebuild')
                  AND id != $1
            )
            "#,
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        if has_running_rebuild {
            tx.rollback().await?;
            return Ok(None);
        }
    }

    sqlx::query(
        "UPDATE index_jobs SET status = 'running', started_at = NOW(), error_opt = NULL WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Some((id, library_id)))
}

pub async fn fail_job(pool: &PgPool, job_id: Uuid, error_message: &str) -> Result<()> {
    sqlx::query(
        "UPDATE index_jobs SET status = 'failed', finished_at = NOW(), error_opt = $2 WHERE id = $1",
    )
    .bind(job_id)
    .bind(error_message)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn is_job_cancelled(pool: &PgPool, job_id: Uuid) -> Result<bool> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(pool)
            .await?;

    Ok(status.as_deref() == Some("cancelled"))
}

pub async fn process_job(
    state: &AppState,
    job_id: Uuid,
    target_library_id: Option<Uuid>,
) -> Result<()> {
    info!("[JOB] Processing {} library={:?}", job_id, target_library_id);

    let (job_type, book_id): (String, Option<Uuid>) = {
        let row = sqlx::query("SELECT type, book_id FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&state.pool)
            .await?;
        (row.get("type"), row.get("book_id"))
    };

    // CBR to CBZ conversion
    if job_type == "cbr_to_cbz" {
        let book_id = book_id.ok_or_else(|| {
            anyhow::anyhow!("cbr_to_cbz job {} has no book_id", job_id)
        })?;
        converter::convert_book(state, job_id, book_id).await?;
        return Ok(());
    }

    // Thumbnail rebuild: generate thumbnails for books missing them
    if job_type == "thumbnail_rebuild" {
        sqlx::query(
            "UPDATE index_jobs SET status = 'generating_thumbnails', started_at = NOW(), phase2_started_at = NOW() WHERE id = $1",
        )
        .bind(job_id)
        .execute(&state.pool)
        .await?;

        analyzer::analyze_library_books(state, job_id, target_library_id, true).await?;

        sqlx::query(
            "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, current_file = NULL WHERE id = $1",
        )
        .bind(job_id)
        .execute(&state.pool)
        .await?;

        return Ok(());
    }

    // Thumbnail regenerate: clear all thumbnails then re-generate
    if job_type == "thumbnail_regenerate" {
        sqlx::query(
            "UPDATE index_jobs SET status = 'generating_thumbnails', started_at = NOW(), phase2_started_at = NOW() WHERE id = $1",
        )
        .bind(job_id)
        .execute(&state.pool)
        .await?;

        analyzer::regenerate_thumbnails(state, job_id, target_library_id).await?;

        sqlx::query(
            "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, current_file = NULL WHERE id = $1",
        )
        .bind(job_id)
        .execute(&state.pool)
        .await?;

        return Ok(());
    }

    let is_full_rebuild = job_type == "full_rebuild";
    info!(
        "[JOB] {} type={} full_rebuild={}",
        job_id, job_type, is_full_rebuild
    );

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
            sqlx::query("DELETE FROM books").execute(&state.pool).await?;
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

    // Count total files for progress estimation
    let library_paths: Vec<String> = libraries
        .iter()
        .map(|library| {
            crate::utils::remap_libraries_path(&library.get::<String, _>("root_path"))
        })
        .collect();

    let total_files: usize = library_paths
        .par_iter()
        .map(|root_path| {
            walkdir::WalkDir::new(root_path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.file_type().is_file()
                        && parsers::detect_format(entry.path()).is_some()
                })
                .count()
        })
        .sum();

    info!(
        "[JOB] Found {} libraries, {} total files to index",
        libraries.len(),
        total_files
    );

    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total_files as i32)
        .execute(&state.pool)
        .await?;

    let mut stats = scanner::JobStats {
        scanned_files: 0,
        indexed_files: 0,
        removed_files: 0,
        errors: 0,
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

    // Sync search index after discovery (books are visible immediately)
    meili::sync_meili(&state.pool, &state.meili_url, &state.meili_master_key).await?;

    // For full rebuild: clean up orphaned thumbnail files (old UUIDs)
    if is_full_rebuild {
        analyzer::cleanup_orphaned_thumbnails(state).await?;
    }

    // Phase 2: Analysis (extract page_count + thumbnails for new/updated books)
    sqlx::query(
        "UPDATE index_jobs SET status = 'generating_thumbnails', phase2_started_at = NOW(), stats_json = $2, current_file = NULL, processed_files = $3 WHERE id = $1",
    )
    .bind(job_id)
    .bind(serde_json::to_value(&stats)?)
    .bind(total_processed_count)
    .execute(&state.pool)
    .await?;

    analyzer::analyze_library_books(state, job_id, target_library_id, false).await?;

    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, current_file = NULL WHERE id = $1",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    Ok(())
}
