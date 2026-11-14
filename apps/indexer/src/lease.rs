//! Liveness of running indexer jobs.
//!
//! `index_jobs` has no heartbeat column, and its timestamps (`started_at`,
//! `phase2_started_at`, ...) say nothing about whether the process running a
//! job is still alive: a scan can legitimately last hours. Each running job is
//! therefore covered by a [`JobLease`]: a session-level PostgreSQL advisory lock
//! held on a dedicated connection for the whole run. PostgreSQL releases the
//! lock as soon as that session ends — process crash, pod kill, lost
//! connection — so "active and unleased" means "orphaned", whatever the age of
//! the job and whichever instance started it.
//!
//! [`fail_unleased_jobs`] fails exactly those jobs. It never touches a job that
//! another live instance is running (rolling deploys, overlapping pods), and it
//! runs periodically, not only at startup, so a job interrupted seconds after it
//! started is reaped too instead of staying `running` forever.

use std::time::Duration;

use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

use stripstream_jobs::{types_for_owner, JobOwner, JobState};

/// Grace period after `started_at` before an unleased job may be failed: covers
/// the instant between `claim_next` (status `running`) and [`JobLease::acquire`].
pub const LEASE_GRACE: Duration = Duration::from_secs(120);

/// How often the worker sweeps orphaned jobs (besides the startup sweep).
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(120);

/// Error written on jobs failed by the sweep.
pub const ORPHANED_JOB_ERROR: &str = "Job interrupted (indexer process no longer running it)";

// Advisory lock key of a job: a fixed namespace plus a hash of its id (the same
// expression in both statements below). A hash collision can only make a dead
// job look alive until the colliding job ends.

/// Take the lease of job `$1`.
const ACQUIRE_SQL: &str =
    "SELECT pg_try_advisory_lock(hashtext('stripstream:index_job_lease'), hashtext($1::text))";

/// Fail active indexer jobs past the grace period whose lease nobody holds.
///
/// The candidates are materialized first (the CTE is never inlined: the lock
/// function is volatile), so the lock is only probed on stale active jobs.
/// `pg_try_advisory_xact_lock` succeeds only if no session holds the lease, and
/// is released at the end of this statement's transaction.
const FAIL_UNLEASED_SQL: &str = "WITH candidates AS MATERIALIZED ( \
         SELECT id FROM index_jobs \
         WHERE status = ANY($2) AND type = ANY($3) \
           AND COALESCE(started_at, created_at) < NOW() - ($4 * INTERVAL '1 second') \
     ), orphaned AS ( \
         SELECT id FROM candidates \
         WHERE pg_try_advisory_xact_lock(hashtext('stripstream:index_job_lease'), hashtext(id::text)) \
     ) \
     UPDATE index_jobs j \
     SET status = 'failed', error_opt = $1, finished_at = NOW() \
     FROM orphaned o \
     WHERE j.id = o.id AND j.status = ANY($2)";

/// Advisory lock held for the duration of a job run. Dropping it closes its
/// dedicated connection, which releases the lock.
pub struct JobLease {
    conn: PoolConnection<Postgres>,
}

impl JobLease {
    /// Take the lease of `job_id` on a connection of its own.
    pub async fn acquire(pool: &PgPool, job_id: Uuid) -> Result<Self, sqlx::Error> {
        let mut conn = pool.acquire().await?;
        // Never hand a session that may hold the lock back to the pool.
        conn.close_on_drop();
        let locked: bool = sqlx::query_scalar(ACQUIRE_SQL)
            .bind(job_id)
            .fetch_one(&mut *conn)
            .await?;
        if !locked {
            return Err(sqlx::Error::Protocol(format!(
                "lease of job {job_id} is already held by another session"
            )));
        }
        Ok(Self { conn })
    }

    /// Release the lease (closes its connection).
    pub async fn release(self) {
        let _ = self.conn.close().await;
    }
}

/// Fail every active indexer job that no live session leases and that started
/// more than `grace` ago. Jobs owned by the API are never touched.
pub async fn fail_unleased_jobs(
    pool: &PgPool,
    grace: Duration,
    error: &str,
) -> Result<u64, sqlx::Error> {
    let types = types_for_owner(JobOwner::Indexer);
    if types.is_empty() {
        return Ok(0);
    }
    let active: Vec<String> = JobState::ACTIVE
        .iter()
        .map(|state| state.as_str().to_string())
        .collect();

    let result = sqlx::query(FAIL_UNLEASED_SQL)
        .bind(error)
        .bind(&active)
        .bind(&types)
        .bind(grace.as_secs() as i64)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn insert_job(pool: &PgPool, job_type: &str, status: &str, age_secs: i64) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, type, status, started_at) \
             VALUES ($1, $2, $3, NOW() - ($4 * INTERVAL '1 second'))",
        )
        .bind(id)
        .bind(job_type)
        .bind(status)
        .bind(age_secs)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn status(pool: &PgPool, id: Uuid) -> String {
        sqlx::query_scalar("SELECT status FROM index_jobs WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// A job still leased by a live process (another pod, a long scan) is never failed,
    /// however old; an unleased one is, even if it was interrupted right after starting.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn only_unleased_jobs_past_the_grace_period_are_failed(pool: PgPool) {
        let leased = insert_job(&pool, "rebuild", "running", 6 * 3600).await;
        let lease = JobLease::acquire(&pool, leased).await.unwrap();
        let mut orphans = Vec::new();
        for state in ["running", "extracting_pages", "generating_thumbnails"] {
            orphans.push(insert_job(&pool, "rebuild", state, 180).await);
        }
        let just_claimed = insert_job(&pool, "cbr_to_cbz", "running", 5).await;
        let pending = insert_job(&pool, "rebuild", "pending", 3600).await;
        let api_job = insert_job(&pool, "metadata_refresh", "running", 3600).await;

        let failed = fail_unleased_jobs(&pool, LEASE_GRACE, ORPHANED_JOB_ERROR)
            .await
            .unwrap();

        assert_eq!(failed, orphans.len() as u64);
        for id in orphans {
            assert_eq!(status(&pool, id).await, "failed");
        }
        assert_eq!(status(&pool, leased).await, "running");
        assert_eq!(status(&pool, just_claimed).await, "running");
        assert_eq!(status(&pool, pending).await, "pending");
        assert_eq!(status(&pool, api_job).await, "running");

        // Once the process holding the lease is gone, the next sweep reaps the job.
        lease.release().await;
        assert_eq!(
            fail_unleased_jobs(&pool, LEASE_GRACE, ORPHANED_JOB_ERROR)
                .await
                .unwrap(),
            1
        );
        assert_eq!(status(&pool, leased).await, "failed");
    }

    /// Dropping a lease (panic, early return) releases it as well.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_dropped_lease_is_released(pool: PgPool) {
        let job = insert_job(&pool, "rebuild", "running", 600).await;
        let lease = JobLease::acquire(&pool, job).await.unwrap();
        assert!(JobLease::acquire(&pool, job).await.is_err());
        drop(lease);

        // The connection closes asynchronously: wait for the lock to be free.
        let mut reacquired = None;
        for _ in 0..50 {
            if let Ok(lease) = JobLease::acquire(&pool, job).await {
                reacquired = Some(lease);
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(reacquired.is_some(), "lease still held after drop");
    }
}
