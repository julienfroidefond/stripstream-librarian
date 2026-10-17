//! Indexer job queue: depth, stuck jobs, failures and per-type breakdown.

use sqlx::Row;

use super::{FailedJob, JobDiagnostics, JobTypeDiagnostics};

/// A pending job older than this is considered stuck: the scheduler's
/// `NOT EXISTS (… status IN ('pending','running'))` guard treats it as
/// in-flight, so nothing new gets planned for that library.
pub(crate) const STALE_PENDING_MINUTES: i32 = 30;

pub(super) async fn jobs_diagnostics(pool: &sqlx::PgPool) -> Result<JobDiagnostics, sqlx::Error> {
    let counts = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE status = 'pending') AS pending,
            COUNT(*) FILTER (WHERE status = 'running') AS running,
            COUNT(*) FILTER (WHERE status = 'failed'
                              AND finished_at > NOW() - INTERVAL '24 hours')
                AS failed_last_24h,
            COUNT(*) FILTER (WHERE status = 'pending'
                              AND created_at < NOW() - make_interval(mins => $1))
                AS stale_pending,
            EXTRACT(EPOCH FROM (
                NOW() - MIN(created_at) FILTER (WHERE status = 'pending')
            ))::bigint AS oldest_pending_seconds
        FROM index_jobs
        "#,
    )
    .bind(STALE_PENDING_MINUTES)
    .fetch_one(pool)
    .await?;

    let by_type = sqlx::query(
        r#"
        SELECT
            type AS job_type,
            COUNT(*) FILTER (WHERE status = 'pending') AS pending,
            COUNT(*) FILTER (WHERE status = 'running') AS running,
            COUNT(*) FILTER (WHERE status = 'failed'
                              AND finished_at > NOW() - INTERVAL '24 hours')
                AS failed_last_24h,
            MAX(finished_at) AS last_finished_at
        FROM index_jobs
        GROUP BY type
        ORDER BY COUNT(*) FILTER (WHERE status IN ('pending', 'running')) DESC,
                 MAX(created_at) DESC
        LIMIT 20
        "#,
    )
    .fetch_all(pool)
    .await?;

    let failures = sqlx::query(
        r#"
        SELECT
            j.id,
            j.type AS job_type,
            l.name AS library_name,
            j.finished_at,
            COALESCE(
                NULLIF(j.error_opt, ''),
                (SELECT e.error_message
                   FROM index_job_errors e
                  WHERE e.job_id = j.id
                  ORDER BY e.created_at DESC
                  LIMIT 1)
            ) AS error
        FROM index_jobs j
        LEFT JOIN libraries l ON l.id = j.library_id
        WHERE j.status = 'failed'
        ORDER BY j.finished_at DESC NULLS LAST, j.created_at DESC
        LIMIT 10
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(JobDiagnostics {
        total: counts.get("total"),
        pending: counts.get("pending"),
        running: counts.get("running"),
        failed_last_24h: counts.get("failed_last_24h"),
        stale_pending: counts.get("stale_pending"),
        oldest_pending_seconds: counts.get("oldest_pending_seconds"),
        by_type: by_type
            .into_iter()
            .map(|row| JobTypeDiagnostics {
                job_type: row.get("job_type"),
                pending: row.get("pending"),
                running: row.get("running"),
                failed_last_24h: row.get("failed_last_24h"),
                last_finished_at: row.get("last_finished_at"),
            })
            .collect(),
        recent_failures: failures
            .into_iter()
            .map(|row| FailedJob {
                id: row.get("id"),
                job_type: row.get("job_type"),
                library_name: row.get("library_name"),
                finished_at: row.get("finished_at"),
                error: row.get("error"),
            })
            .collect(),
    })
}
