//! The single launcher of in-process background jobs started by the API.
//!
//! A handler that starts a job running inside the API process (metadata
//! batch/refresh, reading-status match/push/pull, download detection, RSS
//! poll) describes it with a [`JobSpec`] and calls [`launch`] (one target) or
//! [`launch_each`] (fan-out over libraries). The launcher owns the whole
//! sequence: conflict check, `index_jobs` insertion (directly `running`, so the
//! poller never claims it), `tokio::spawn`, and on failure the partial stats,
//! `fail_job` and the failure notification. The job poller reuses the same
//! failure path through [`report_failure`].

use std::future::Future;

use futures::future::BoxFuture;
use notifications::NotificationEvent;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use tracing::warn;
use utoipa::ToSchema;
use uuid::Uuid;

use stripstream_jobs::status::JobState;

use crate::error::ApiError;
use crate::jobs::lifecycle::fail_job;

/// Builds the failure notification from the library name and the error.
pub(crate) type FailureEvent = fn(Option<String>, String) -> NotificationEvent;

/// Rebuilds the `stats_json` reached so far, stored alongside the failure.
pub(crate) type PartialStats = fn(PgPool, Uuid) -> BoxFuture<'static, Value>;

/// Static description of a job type the API launches in-process.
#[derive(Clone, Copy)]
pub(crate) struct JobSpec {
    /// Value written in `index_jobs.type`.
    pub(crate) job_type: &'static str,
    /// Other job types that must not run at the same time on the same target.
    /// `job_type` itself always conflicts and need not be listed.
    pub(crate) conflicts: &'static [&'static str],
    /// Notification sent when the job fails, if any.
    pub(crate) failure: Option<FailureEvent>,
    /// Partial stats stored by `fail_job`; `None` keeps the stored stats.
    pub(crate) partial_stats: Option<PartialStats>,
}

impl JobSpec {
    /// Every job type whose in-flight presence blocks a new launch.
    pub(crate) fn blocking_types(&self) -> Vec<&'static str> {
        std::iter::once(self.job_type)
            .chain(self.conflicts.iter().copied())
            .collect()
    }
}

/// Outcome of a single launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Launch {
    Started(Uuid),
    AlreadyRunning(Uuid),
}

/// Outcome of a fan-out over libraries.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct FanOut {
    /// Jobs created, in launch order.
    pub(crate) started: Vec<Uuid>,
    /// Jobs already in flight that made a library skip.
    pub(crate) already_running: Vec<Uuid>,
}

/// Status reported by the job-launch endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobLaunchStatus {
    /// One job was created and is running.
    Running,
    /// A conflicting job is already in flight; `id` is that job.
    AlreadyRunning,
    /// Fan-out over libraries; `id` is the last created job, or `null`.
    Started,
}

/// Response of every endpoint that launches a background job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct JobLaunchResponse {
    #[schema(value_type = Option<String>)]
    pub id: Option<Uuid>,
    pub status: JobLaunchStatus,
}

impl From<Launch> for JobLaunchResponse {
    fn from(launch: Launch) -> Self {
        match launch {
            Launch::Started(id) => Self {
                id: Some(id),
                status: JobLaunchStatus::Running,
            },
            Launch::AlreadyRunning(id) => Self {
                id: Some(id),
                status: JobLaunchStatus::AlreadyRunning,
            },
        }
    }
}

impl From<FanOut> for JobLaunchResponse {
    fn from(fan_out: FanOut) -> Self {
        Self {
            id: fan_out.started.last().copied(),
            status: JobLaunchStatus::Started,
        }
    }
}

/// Launch one job of `spec` on `library_id` (`None` = a global job).
///
/// Returns the in-flight job instead when one of `spec`'s blocking types is
/// already running on the same target.
pub(crate) async fn launch<F, Fut>(
    pool: &PgPool,
    library_id: Option<Uuid>,
    spec: &JobSpec,
    run: F,
) -> Result<Launch, ApiError>
where
    F: FnOnce(PgPool, Uuid) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    launch_with(pool, library_id, spec, run, notifications::notify).await
}

/// Launch one job of `spec` per library, skipping the libraries where a
/// blocking job is already in flight. `run` receives `(pool, job_id, library_id)`.
pub(crate) async fn launch_each<I, F, Fut>(
    pool: &PgPool,
    library_ids: I,
    spec: &JobSpec,
    run: F,
) -> Result<FanOut, ApiError>
where
    I: IntoIterator<Item = Uuid>,
    F: Fn(PgPool, Uuid, Uuid) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    launch_each_with(pool, library_ids, spec, run, notifications::notify).await
}

/// Failure path shared by launched and polled jobs: partial stats, `fail_job`
/// and the failure notification.
pub(crate) async fn report_failure(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Option<Uuid>,
    spec: &JobSpec,
    error: &str,
) {
    report_failure_with(pool, job_id, library_id, spec, error, notifications::notify).await
}

pub(crate) async fn launch_with<F, Fut, N>(
    pool: &PgPool,
    library_id: Option<Uuid>,
    spec: &JobSpec,
    run: F,
    notify: N,
) -> Result<Launch, ApiError>
where
    F: FnOnce(PgPool, Uuid) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
    N: Fn(PgPool, NotificationEvent) + Send + 'static,
{
    let job_id = match insert_unless_in_flight(pool, library_id, spec).await? {
        Launch::Started(id) => id,
        running @ Launch::AlreadyRunning(_) => return Ok(running),
    };

    let pool = pool.clone();
    let spec = *spec;
    tokio::spawn(async move {
        if let Err(error) = run(pool.clone(), job_id).await {
            report_failure_with(&pool, job_id, library_id, &spec, &error, notify).await;
        }
    });

    Ok(Launch::Started(job_id))
}

/// Check-then-insert of a launch, atomically.
///
/// A transaction-scoped advisory lock per target (library, or the global
/// scope) serializes concurrent launches, so two requests can never both see
/// "nothing in flight" and insert twice. The job is inserted directly as
/// `running` so the poller never claims it.
async fn insert_unless_in_flight(
    pool: &PgPool,
    library_id: Option<Uuid>,
    spec: &JobSpec,
) -> Result<Launch, sqlx::Error> {
    let in_flight: Vec<String> = JobState::IN_FLIGHT
        .iter()
        .map(|s| s.as_str().to_string())
        .collect();
    let blocking: Vec<String> = spec
        .blocking_types()
        .into_iter()
        .map(str::to_string)
        .collect();

    let mut tx = pool.begin().await?;
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtext('index_jobs_launch:' || COALESCE($1::text, 'global')))",
    )
    .bind(library_id)
    .execute(&mut *tx)
    .await?;

    // Same scope rule as `job_in_flight` (Library / Global).
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs \
         WHERE library_id IS NOT DISTINCT FROM $1 \
           AND type = ANY($2) \
           AND status = ANY($3) \
         LIMIT 1",
    )
    .bind(library_id)
    .bind(&blocking)
    .bind(&in_flight)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(existing) = existing {
        return Ok(Launch::AlreadyRunning(existing));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) \
         VALUES ($1, $2, $3, 'running', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .bind(spec.job_type)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Launch::Started(job_id))
}

pub(crate) async fn launch_each_with<I, F, Fut, N>(
    pool: &PgPool,
    library_ids: I,
    spec: &JobSpec,
    run: F,
    notify: N,
) -> Result<FanOut, ApiError>
where
    I: IntoIterator<Item = Uuid>,
    F: Fn(PgPool, Uuid, Uuid) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
    N: Fn(PgPool, NotificationEvent) + Clone + Send + 'static,
{
    let mut fan_out = FanOut::default();
    for library_id in library_ids {
        let run = run.clone();
        let launched = launch_with(
            pool,
            Some(library_id),
            spec,
            move |pool, job_id| run(pool, job_id, library_id),
            notify.clone(),
        )
        .await?;
        match launched {
            Launch::Started(id) => fan_out.started.push(id),
            Launch::AlreadyRunning(id) => fan_out.already_running.push(id),
        }
    }
    Ok(fan_out)
}

async fn report_failure_with<N>(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Option<Uuid>,
    spec: &JobSpec,
    error: &str,
    notify: N,
) where
    N: Fn(PgPool, NotificationEvent),
{
    warn!(
        "[JOB_LAUNCHER] {} job {job_id} failed: {error}",
        spec.job_type
    );

    let stats = match spec.partial_stats {
        Some(build) => Some(build(pool.clone(), job_id).await),
        None => None,
    };
    if let Err(e) = fail_job(pool, job_id, error, stats).await {
        warn!("[JOB_LAUNCHER] could not mark job {job_id} as failed: {e}");
    }

    if let Some(event) = spec.failure {
        let library_name = library_name(pool, library_id).await;
        notify(pool.clone(), event(library_name, error.to_string()));
    }
}

async fn library_name(pool: &PgPool, library_id: Option<Uuid>) -> Option<String> {
    let library_id = library_id?;
    sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}
