//! Terminal transitions for `index_jobs`.
//!
//! Cancellation wins over a late success/failure: every terminal transition
//! refuses to overwrite a `cancelled` row.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::status::{as_strings, JobState};

/// Terminal status written when a job finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Success,
    Partial,
    Failed,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Success => "success",
            JobStatus::Partial => "partial",
            JobStatus::Failed => "failed",
        }
    }
}

/// Map the outcome of a job to its terminal status.
///
/// * no failure -> `success`
/// * some failures, but some units succeeded -> `partial`
/// * every attempted unit failed (or failures with nothing attempted) -> `failed`
pub fn classify_job(attempted: usize, errors: usize) -> JobStatus {
    if errors == 0 {
        JobStatus::Success
    } else if errors >= attempted {
        JobStatus::Failed
    } else {
        JobStatus::Partial
    }
}

/// How the `library_id` column is filtered when looking up an in-flight job.
#[derive(Debug, Clone, Copy)]
pub enum JobScope {
    /// Do not filter on `library_id` at all.
    Any,
    /// Only jobs belonging to this library.
    Library(Uuid),
    /// Only global jobs (`library_id IS NULL`).
    Global,
}

impl JobScope {
    fn parts(self) -> (&'static str, Option<Uuid>) {
        match self {
            JobScope::Any => ("any", None),
            JobScope::Library(id) => ("library", Some(id)),
            JobScope::Global => ("global", None),
        }
    }
}

/// Return the id of a job of one of `types` that is already in flight
/// (`pending`, `running`, `extracting_pages` or `generating_thumbnails`)
/// within `scope`, if any.
pub async fn job_in_flight(
    pool: &PgPool,
    scope: JobScope,
    types: &[&str],
) -> Result<Option<Uuid>, sqlx::Error> {
    let (scope, library_id) = scope.parts();
    let types: Vec<String> = types.iter().map(|t| (*t).to_string()).collect();

    sqlx::query_scalar(
        "SELECT id FROM index_jobs \
         WHERE ($1 = 'any' \
                OR ($1 = 'library' AND library_id = $2) \
                OR ($1 = 'global' AND library_id IS NULL)) \
           AND type = ANY($3) \
           AND status = ANY($4) \
         LIMIT 1",
    )
    .bind(scope)
    .bind(library_id)
    .bind(types)
    .bind(as_strings(&JobState::IN_FLIGHT))
    .fetch_optional(pool)
    .await
}

/// Mark a job as `failed`, optionally merging a `stats_json` payload (existing
/// stats are preserved when `stats` is `None`). A cancelled job is left untouched.
pub async fn fail_job(
    pool: &PgPool,
    job_id: Uuid,
    error: &str,
    stats: Option<Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'failed', error_opt = $2, finished_at = NOW(), \
             stats_json = COALESCE($3, stats_json) \
         WHERE id = $1 AND status <> 'cancelled'",
    )
    .bind(job_id)
    .bind(error)
    .bind(stats)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Finish a job with an explicit terminal status.
///
/// `error` is stored as-is (use `None` for a clean run) and `progress_percent`
/// is set to 100 except for `failed`, which keeps whatever progress the job had
/// reached. Callers pass a human-readable summary for `partial` runs so the UI
/// can explain what went wrong without parsing `stats_json`.
///
/// Cancellation wins: a `cancelled` row is never overwritten.
pub async fn finish_job(
    pool: &PgPool,
    job_id: Uuid,
    status: JobStatus,
    stats: Value,
    error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = $2, finished_at = NOW(), \
             stats_json = $3, error_opt = $4, \
             progress_percent = CASE WHEN $2 = 'failed' \
                                      THEN progress_percent ELSE 100 END \
         WHERE id = $1 AND status <> 'cancelled'",
    )
    .bind(job_id)
    .bind(status.as_str())
    .bind(stats)
    .bind(error)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Mark a job as `success` and store its final `stats_json` payload.
pub async fn complete_job(pool: &PgPool, job_id: Uuid, stats: Value) -> Result<(), sqlx::Error> {
    finish_job(pool, job_id, JobStatus::Success, stats, None).await
}

/// Mark a job as `success` without touching its stats.
pub async fn finish_success(pool: &PgPool, job_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'success', finished_at = NOW(), progress_percent = 100, current_file = NULL \
         WHERE id = $1 AND status <> 'cancelled'",
    )
    .bind(job_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Mark a claimed (or directly created) job as `running`.
///
/// Late calls after cancellation are ignored so a cancelled job is never
/// resurrected.
pub async fn start_job(pool: &PgPool, job_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'running', started_at = NOW(), error_opt = NULL \
         WHERE id = $1 AND status IN ('pending', 'running')",
    )
    .bind(job_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Cancel an in-flight job. Returns the number of rows changed (0 when the job
/// is already terminal, which callers surface as "not found or already finished").
pub async fn cancel_job(pool: &PgPool, job_id: Uuid) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE index_jobs \
         SET status = 'cancelled', finished_at = NOW() \
         WHERE id = $1 AND status = ANY($2)",
    )
    .bind(job_id)
    .bind(as_strings(&JobState::IN_FLIGHT))
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Thumbnail-only jobs (`thumbnail_rebuild`, `thumbnail_regenerate`) start
/// directly in the thumbnail phase with both timestamps set.
pub async fn begin_thumbnail_job(pool: &PgPool, job_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'generating_thumbnails', started_at = NOW(), \
             phase2_started_at = NOW(), current_file = NULL \
         WHERE id = $1 AND status = ANY($2)",
    )
    .bind(job_id)
    .bind(as_strings(&JobState::IN_FLIGHT))
    .execute(pool)
    .await
    .map(|_| ())
}

/// Enter the page-extraction sub-phase (sub-phase A).
pub async fn begin_extraction(pool: &PgPool, job_id: Uuid, total: i32) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'extracting_pages', total_files = $2, processed_files = 0, \
             current_file = NULL \
         WHERE id = $1 AND status = ANY($3)",
    )
    .bind(job_id)
    .bind(total)
    .bind(as_strings(&JobState::IN_FLIGHT))
    .execute(pool)
    .await
    .map(|_| ())
}

/// Enter the thumbnail-generation sub-phase (sub-phase B).
pub async fn begin_thumbnail_generation(
    pool: &PgPool,
    job_id: Uuid,
    total: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'generating_thumbnails', generating_thumbnails_started_at = NOW(), \
             total_files = $2, processed_files = 0, current_file = NULL \
         WHERE id = $1 AND status = ANY($3)",
    )
    .bind(job_id)
    .bind(total)
    .bind(as_strings(&JobState::IN_FLIGHT))
    .execute(pool)
    .await
    .map(|_| ())
}

/// Enter the analysis phase after discovery, persisting the scan stats first.
pub async fn begin_analysis(
    pool: &PgPool,
    job_id: Uuid,
    stats: Value,
    processed: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'generating_thumbnails', phase2_started_at = NOW(), stats_json = $2, \
             current_file = NULL, processed_files = $3 \
         WHERE id = $1 AND status = ANY($4)",
    )
    .bind(job_id)
    .bind(stats)
    .bind(processed)
    .bind(as_strings(&JobState::IN_FLIGHT))
    .execute(pool)
    .await
    .map(|_| ())
}
