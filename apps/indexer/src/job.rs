//! Indexer job runner: one typed outcome per job.
//!
//! [`process_job`] runs a claimed job and returns either a [`JobOutcome`] (the
//! job ran to the end, possibly with failed units) or a [`RunError`]
//! (cancelled, or aborted). [`finish`] turns an outcome into the terminal
//! status of ADR 0002 through `classify_job`: `success`, `partial` or `failed`.

use std::fmt;

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};
use tracing::{error, info};
use uuid::Uuid;

use stripstream_jobs::{
    begin_analysis, begin_thumbnail_job, classify_job, cleanup_stale_jobs as cleanup_owner_jobs,
    finish_job, purge_terminal_jobs, set_total_files, JobOwner, JobStatus,
};

use crate::{analyzer, converter, lease, scanner, AppState};

/// Error raised by the scanner and the analyzer when the user cancelled the job.
///
/// Cancellation is recognised by this type only, never by an error message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobCancelled;

impl fmt::Display for JobCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Job cancelled by user")
    }
}

impl std::error::Error for JobCancelled {}

/// Whether `err` (or an error it wraps with context) is a [`JobCancelled`].
pub fn is_cancellation(err: &anyhow::Error) -> bool {
    err.downcast_ref::<JobCancelled>().is_some()
}

/// Why a job stopped without an outcome.
#[derive(Debug)]
pub enum RunError {
    /// The user cancelled the job: its row is already `cancelled`.
    Cancelled,
    /// The job could not run to the end: its row must be marked `failed`.
    Failed(anyhow::Error),
}

impl From<anyhow::Error> for RunError {
    fn from(err: anyhow::Error) -> Self {
        if is_cancellation(&err) {
            Self::Cancelled
        } else {
            Self::Failed(err)
        }
    }
}

/// What a job that ran to the end did.
#[derive(Debug, Clone, PartialEq)]
pub struct JobOutcome {
    /// Units the job attempted (libraries for a scan, 1 for single-unit jobs).
    pub attempted: usize,
    /// Units that failed.
    pub errors: usize,
    /// New `stats_json`; `None` keeps what the job stored while running.
    pub stats: Option<Value>,
}

impl JobOutcome {
    /// A job made of a single unit that succeeded (conversion, thumbnails).
    pub fn single_success() -> Self {
        Self {
            attempted: 1,
            errors: 0,
            stats: None,
        }
    }

    /// Terminal status: `success`, `partial` or `failed` (ADR 0002).
    pub fn status(&self) -> JobStatus {
        classify_job(self.attempted, self.errors)
    }

    /// Human-readable summary stored in `error_opt` when some units failed.
    pub fn error_summary(&self) -> Option<String> {
        (self.errors > 0).then(|| format!("{}/{} unit(s) failed", self.errors, self.attempted))
    }
}

/// Write the terminal status of a job that ran to the end and return it.
///
/// A job cancelled in the meantime stays `cancelled` (`finish_job` never
/// overwrites a cancellation).
pub async fn finish(
    pool: &PgPool,
    job_id: Uuid,
    outcome: &JobOutcome,
) -> Result<JobStatus, sqlx::Error> {
    let status = outcome.status();
    finish_job(
        pool,
        job_id,
        status,
        outcome.stats.clone(),
        outcome.error_summary().as_deref(),
    )
    .await?;
    Ok(status)
}

/// Fold one library's discovery result into the scan: a cancellation stops the
/// job, any other error counts against that library and the scan goes on.
fn fold_library_scan(
    library_id: Uuid,
    result: Result<()>,
    stats: &mut scanner::JobStats,
) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(err) if is_cancellation(&err) => Err(err),
        Err(err) => {
            stats.errors += 1;
            error!(library_id = %library_id, error = %err, "library scan failed");
            Ok(())
        }
    }
}

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

/// Startup cleanup (the active-job half also runs periodically, see
/// [`sweep_orphaned_jobs`]).
///
/// Active (`running` / `extracting_pages` / `generating_thumbnails`) indexer
/// jobs are failed only when no live process holds their [`lease`](crate::lease):
/// during a rolling deploy or with two overlapping instances, the jobs of the
/// other instance are left alone. API-owned job types are never touched.
pub async fn cleanup_stale_jobs(pool: &PgPool) -> Result<()> {
    sweep_orphaned_jobs(pool).await?;

    let report = cleanup_owner_jobs(
        pool,
        JobOwner::Indexer,
        None,
        Some(std::time::Duration::from_secs(30 * 60)),
        "Job interrupted by indexer restart",
        "Job was never picked up (stale pending)",
    )
    .await?;

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

/// Fail active indexer jobs whose process is gone (no lease held), whatever their
/// age, once they are past [`lease::LEASE_GRACE`]. Runs at startup and every
/// [`lease::SWEEP_INTERVAL`], so a job interrupted right after it started does
/// not stay active forever.
pub async fn sweep_orphaned_jobs(pool: &PgPool) -> Result<u64> {
    let orphaned =
        lease::fail_unleased_jobs(pool, lease::LEASE_GRACE, lease::ORPHANED_JOB_ERROR).await?;
    if orphaned > 0 {
        info!("[CLEANUP] Marked {orphaned} interrupted job(s) as failed");
    }
    Ok(orphaned)
}

/// Run a claimed job to its outcome. The caller writes the terminal status
/// ([`finish`] on success, `fail_job` on [`RunError::Failed`]).
pub async fn process_job(
    state: &AppState,
    job_id: Uuid,
    target_library_id: Option<Uuid>,
) -> Result<JobOutcome, RunError> {
    run_job(state, job_id, target_library_id)
        .await
        .map_err(RunError::from)
}

async fn run_job(
    state: &AppState,
    job_id: Uuid,
    target_library_id: Option<Uuid>,
) -> Result<JobOutcome> {
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
        return Ok(JobOutcome::single_success());
    }

    // Thumbnail rebuild: generate thumbnails for books missing them
    if job_type == "thumbnail_rebuild" {
        begin_thumbnail_job(&state.pool, job_id).await?;
        analyzer::analyze_library_books(state, job_id, target_library_id, true).await?;
        return Ok(JobOutcome::single_success());
    }

    // Thumbnail regenerate: clear all thumbnails then re-generate
    if job_type == "thumbnail_regenerate" {
        begin_thumbnail_job(&state.pool, job_id).await?;
        analyzer::regenerate_thumbnails(state, job_id, target_library_id).await?;
        return Ok(JobOutcome::single_success());
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
                state
                    .library_root
                    .resolve(&library.get::<String, _>("root_path"))
                    .map(|path| path.to_local().to_string())
                    .map_err(anyhow::Error::from)
            })
            .collect::<anyhow::Result<_>>()?;
        // Count sequentially with limited open fds to avoid ENFILE exhaustion
        library_paths
            .iter()
            .map(|root_path| {
                walkdir::WalkDir::new(root_path)
                    .follow_links(scanner::FOLLOW_SYMLINKS)
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
        let root_path = state
            .library_root
            .resolve(&root_path)?
            .to_local()
            .to_string();
        let result = scanner::scan_library_discovery(
            state,
            job_id,
            library_id,
            std::path::Path::new(&root_path),
            &mut stats,
            &mut total_processed_count,
            total_files,
            is_full_rebuild,
        )
        .await;
        fold_library_scan(library_id, result, &mut stats)?;
    }

    // For full rebuild: clean up orphaned thumbnail files (old UUIDs)
    // Housekeeping only: when it cannot run safely it is skipped, the rebuild goes on.
    if is_full_rebuild {
        if let Err(e) = analyzer::cleanup_orphaned_thumbnails(state).await {
            error!("[JOB] Orphan thumbnail cleanup skipped: {e:#}");
        }
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

    Ok(JobOutcome {
        attempted: libraries.len(),
        errors: stats.errors,
        stats: Some(serde_json::to_value(&stats)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_stats() -> scanner::JobStats {
        scanner::JobStats {
            scanned_files: 0,
            indexed_files: 0,
            removed_files: 0,
            errors: 0,
            warnings: 0,
            new_series: 0,
            new_series_names: Vec::new(),
            new_book_titles: Vec::new(),
        }
    }

    fn outcome(attempted: usize, errors: usize) -> JobOutcome {
        JobOutcome {
            attempted,
            errors,
            stats: None,
        }
    }

    #[test]
    fn an_error_mentioning_cancelled_is_not_a_cancellation() {
        for message in ["Job cancelled by user", "upstream request Cancelled"] {
            assert!(
                matches!(
                    RunError::from(anyhow::anyhow!("{message}")),
                    RunError::Failed(_)
                ),
                "{message:?} is a failure, not a cancellation"
            );
        }
    }

    #[test]
    fn a_typed_cancellation_is_recognised_even_with_context() {
        assert!(matches!(
            RunError::from(anyhow::Error::new(JobCancelled)),
            RunError::Cancelled
        ));
        let wrapped = anyhow::Error::new(JobCancelled).context("scanning library");
        assert!(matches!(RunError::from(wrapped), RunError::Cancelled));
    }

    #[test]
    fn a_failed_library_counts_and_the_scan_goes_on() {
        let mut stats = empty_stats();
        let library = Uuid::new_v4();

        fold_library_scan(library, Ok(()), &mut stats).unwrap();
        fold_library_scan(
            library,
            Err(anyhow::anyhow!("walk cancelled by the filesystem")),
            &mut stats,
        )
        .unwrap();

        assert_eq!(stats.errors, 1);
    }

    #[test]
    fn a_cancelled_library_stops_the_scan_without_counting() {
        let mut stats = empty_stats();

        let err = fold_library_scan(
            Uuid::new_v4(),
            Err(anyhow::Error::new(JobCancelled)),
            &mut stats,
        )
        .unwrap_err();

        assert!(is_cancellation(&err));
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn outcome_status_follows_attempted_and_failed_units() {
        assert_eq!(outcome(0, 0).status(), JobStatus::Success);
        assert_eq!(outcome(2, 0).status(), JobStatus::Success);
        assert_eq!(outcome(2, 1).status(), JobStatus::Partial);
        assert_eq!(outcome(2, 2).status(), JobStatus::Failed);
        assert_eq!(outcome(2, 0).error_summary(), None);
        assert_eq!(
            outcome(2, 1).error_summary().as_deref(),
            Some("1/2 unit(s) failed")
        );
    }

    async fn insert_job(pool: &PgPool, job_type: &str, status: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO index_jobs (id, type, status) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(job_type)
            .bind(status)
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn job_status(pool: &PgPool, job_id: Uuid) -> (String, Option<String>) {
        sqlx::query_as("SELECT status, error_opt FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn finish_writes_partial_and_failed_scans(pool: PgPool) {
        let partial = insert_job(&pool, "rebuild", "generating_thumbnails").await;
        let failed = insert_job(&pool, "rebuild", "generating_thumbnails").await;

        assert_eq!(
            finish(&pool, partial, &outcome(2, 1)).await.unwrap(),
            JobStatus::Partial
        );
        assert_eq!(
            finish(&pool, failed, &outcome(2, 2)).await.unwrap(),
            JobStatus::Failed
        );

        assert_eq!(
            job_status(&pool, partial).await,
            (
                "partial".to_string(),
                Some("1/2 unit(s) failed".to_string())
            )
        );
        assert_eq!(
            job_status(&pool, failed).await,
            ("failed".to_string(), Some("2/2 unit(s) failed".to_string()))
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn finish_keeps_a_cancellation(pool: PgPool) {
        let job = insert_job(&pool, "rebuild", "cancelled").await;

        finish(&pool, job, &outcome(1, 0)).await.unwrap();

        assert_eq!(job_status(&pool, job).await.0, "cancelled");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_scan_of_an_empty_library_succeeds(pool: PgPool) {
        let root = tempfile::tempdir().unwrap();
        let library = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'lib', $2)")
            .bind(library)
            .bind(root.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .unwrap();
        let job = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) \
             VALUES ($1, $2, 'rebuild', 'running')",
        )
        .bind(job)
        .bind(library)
        .execute(&pool)
        .await
        .unwrap();
        let state = crate::test_state(pool.clone());

        let outcome = process_job(&state, job, Some(library)).await.unwrap();
        assert_eq!((outcome.attempted, outcome.errors), (1, 0));
        assert_eq!(
            finish(&pool, job, &outcome).await.unwrap(),
            JobStatus::Success
        );

        let (status, error) = job_status(&pool, job).await;
        assert_eq!((status.as_str(), error), ("success", None));
        let stats: Value = sqlx::query_scalar("SELECT stats_json FROM index_jobs WHERE id = $1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stats["errors"], 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_thumbnail_job_is_a_single_unit(pool: PgPool) {
        let job = insert_job(&pool, "thumbnail_rebuild", "running").await;
        let state = crate::test_state(pool.clone());

        let outcome = process_job(&state, job, None).await.unwrap();

        assert_eq!(outcome, JobOutcome::single_success());
    }

    /// Startup cleanup fails the active indexer jobs nobody runs any more (past the lease
    /// grace period, even if they had just started), keeps the ones a live instance leases,
    /// and never touches API jobs.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn startup_cleanup_fails_only_unleased_indexer_jobs(pool: PgPool) {
        let past_grace = |id: Uuid| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "UPDATE index_jobs SET started_at = NOW() - INTERVAL '3 minutes' WHERE id = $1",
                )
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        let mut orphans = Vec::new();
        for status in ["running", "extracting_pages", "generating_thumbnails"] {
            let id = insert_job(&pool, "rebuild", status).await;
            past_grace(id).await;
            orphans.push(id);
        }
        let conversion = insert_job(&pool, "cbr_to_cbz", "running").await;
        past_grace(conversion).await;
        let other_instance = insert_job(&pool, "rebuild", "running").await;
        past_grace(other_instance).await;
        let lease = lease::JobLease::acquire(&pool, other_instance)
            .await
            .unwrap();
        let fresh_pending = insert_job(&pool, "rebuild", "pending").await;
        let api_job = insert_job(&pool, "metadata_refresh", "running").await;
        past_grace(api_job).await;

        cleanup_stale_jobs(&pool).await.unwrap();

        for id in orphans.into_iter().chain([conversion]) {
            assert_eq!(
                job_status(&pool, id).await,
                (
                    "failed".to_string(),
                    Some(lease::ORPHANED_JOB_ERROR.to_string())
                )
            );
        }
        assert_eq!(job_status(&pool, other_instance).await.0, "running");
        assert_eq!(job_status(&pool, fresh_pending).await.0, "pending");
        assert_eq!(job_status(&pool, api_job).await.0, "running");
        lease.release().await;
    }
}
