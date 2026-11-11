//! Owner-scoped stale-job cleanup and terminal retention.

use std::time::Duration;

use sqlx::PgPool;

use crate::registry::{types_for_owner, JobOwner};
use crate::status::{as_strings, JobState};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub active: u64,
    pub pending: u64,
}

fn seconds(value: Duration) -> i64 {
    value.as_secs().max(1) as i64
}

/// Fail stale jobs owned by `owner`.
///
/// - active jobs (running / extracting / generating) older than `active_after`
/// - pending jobs older than `pending_after`
///
/// Thresholds are optional so a service can run only the relevant half.
/// Jobs owned by other services are never touched.
pub async fn cleanup_stale_jobs(
    pool: &PgPool,
    owner: JobOwner,
    active_after: Option<Duration>,
    pending_after: Option<Duration>,
    active_error: &str,
    pending_error: &str,
) -> Result<CleanupReport, sqlx::Error> {
    let types = types_for_owner(owner);
    if types.is_empty() {
        return Ok(CleanupReport::default());
    }

    let mut report = CleanupReport::default();

    if let Some(after) = active_after {
        let result = sqlx::query(
            "UPDATE index_jobs \
             SET status = 'failed', finished_at = NOW(), error_opt = $4 \
             WHERE status = ANY($1) AND type = ANY($2) \
               AND started_at IS NOT NULL \
               AND started_at < NOW() - ($3 * INTERVAL '1 second')",
        )
        .bind(as_strings(&JobState::ACTIVE))
        .bind(&types)
        .bind(seconds(after))
        .bind(active_error)
        .execute(pool)
        .await?;
        report.active = result.rows_affected();
    }

    if let Some(after) = pending_after {
        let result = sqlx::query(
            "UPDATE index_jobs \
             SET status = 'failed', finished_at = NOW(), error_opt = $3 \
             WHERE status = 'pending' AND type = ANY($1) \
               AND created_at < NOW() - ($2 * INTERVAL '1 second')",
        )
        .bind(&types)
        .bind(seconds(after))
        .bind(pending_error)
        .execute(pool)
        .await?;
        report.pending = result.rows_affected();
    }

    Ok(report)
}

/// Fail all active jobs owned by `owner` (used at process startup, when any
/// active row necessarily belongs to a previous process).
pub async fn fail_orphaned_jobs(
    pool: &PgPool,
    owner: JobOwner,
    error: &str,
) -> Result<u64, sqlx::Error> {
    let types = types_for_owner(owner);
    if types.is_empty() {
        return Ok(0);
    }

    let result = sqlx::query(
        "UPDATE index_jobs \
         SET status = 'failed', error_opt = $3, finished_at = NOW() \
         WHERE status = ANY($1) AND type = ANY($2)",
    )
    .bind(as_strings(&JobState::ACTIVE))
    .bind(&types)
    .bind(error)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// Delete terminal jobs older than `retention_days`.
pub async fn purge_terminal_jobs(pool: &PgPool, retention_days: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM index_jobs \
         WHERE status = ANY($1) \
           AND created_at < NOW() - ($2 * INTERVAL '1 day')",
    )
    .bind(as_strings(&JobState::TERMINAL))
    .bind(retention_days.max(0))
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
