//! Shared helpers for the `index_jobs` lifecycle.
//!
//! Every background job (metadata sync, downloads, reading-status, ...) follows the
//! same shape: check whether a job of the same `type` is already in flight, then
//! mark it `failed` or `success` when it finishes. These helpers factor out the
//! corresponding SQL so the semantics stay identical across modules.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

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

/// Return the id of a job of one of `types` that is already `pending` or `running`
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
           AND status IN ('pending', 'running') \
         LIMIT 1",
    )
    .bind(scope)
    .bind(library_id)
    .bind(types)
    .fetch_optional(pool)
    .await
}

/// Mark a job as `failed`, optionally merging a `stats_json` payload (existing
/// stats are preserved when `stats` is `None`).
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
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(error)
    .bind(stats)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Mark a job as `success` and store its final `stats_json` payload.
pub async fn complete_job(pool: &PgPool, job_id: Uuid, stats: Value) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE index_jobs \
         SET status = 'success', finished_at = NOW(), \
             stats_json = $2, progress_percent = 100 \
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(stats)
    .execute(pool)
    .await
    .map(|_| ())
}
