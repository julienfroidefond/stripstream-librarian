use std::time::Duration;

use futures::future::BoxFuture;
use notifications::NotificationEvent;
use sqlx::{PgPool, Row};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::jobs::launcher::{
    launch_each_with, launch_with, FanOut, JobLaunchResponse, JobLaunchStatus, JobSpec, Launch,
};

const TIMEOUT: Duration = Duration::from_secs(10);

fn batch_failed(library_name: Option<String>, error: String) -> NotificationEvent {
    NotificationEvent::MetadataBatchFailed {
        library_name,
        error,
    }
}

fn partial_stats(_pool: PgPool, job_id: Uuid) -> BoxFuture<'static, serde_json::Value> {
    Box::pin(async move { serde_json::json!({ "processed": 3, "job": job_id.to_string() }) })
}

const SPEC: JobSpec = JobSpec {
    job_type: "metadata_batch",
    conflicts: &["metadata_batch_rematch"],
    failure: Some(batch_failed),
    partial_stats: Some(partial_stats),
};

const GLOBAL_SPEC: JobSpec = JobSpec {
    job_type: "prowlarr_rss",
    conflicts: &[],
    failure: None,
    partial_stats: None,
};

async fn create_library(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(format!("/libraries/{name}"))
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn create_job(pool: &PgPool, library_id: Option<Uuid>, job_type: &str, status: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(library_id)
        .bind(job_type)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn job_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A notifier that forwards events to the test instead of Telegram.
fn channel_notifier() -> (
    impl Fn(PgPool, NotificationEvent) + Clone + Send + 'static,
    mpsc::UnboundedReceiver<NotificationEvent>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    (move |_pool, event| tx.send(event).unwrap(), rx)
}

fn no_notifier(_pool: PgPool, _event: NotificationEvent) {
    panic!("no notification expected");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn launch_inserts_a_running_job_and_runs_it(pool: PgPool) {
    let lib = create_library(&pool, "launch_ok").await;
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();

    let launched = launch_with(
        &pool,
        Some(lib),
        &SPEC,
        move |_pool, job_id| async move {
            done_tx.send(job_id).unwrap();
            Ok(())
        },
        no_notifier,
    )
    .await
    .unwrap();

    let Launch::Started(job_id) = launched else {
        panic!("expected a started job, got {launched:?}");
    };
    let ran = tokio::time::timeout(TIMEOUT, done_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ran, job_id, "run receives the created job id");

    let row = sqlx::query("SELECT library_id, type, status, started_at IS NOT NULL AS started FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<Option<Uuid>, _>("library_id"), Some(lib));
    assert_eq!(row.get::<String, _>("type"), "metadata_batch");
    assert_eq!(row.get::<String, _>("status"), "running");
    assert!(row.get::<bool, _>("started"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn launch_returns_a_conflicting_in_flight_job(pool: PgPool) {
    let lib = create_library(&pool, "launch_conflict").await;
    let existing = create_job(&pool, Some(lib), "metadata_batch_rematch", "pending").await;

    let launched = launch_with(
        &pool,
        Some(lib),
        &SPEC,
        |_pool, _job_id| async { panic!("a conflicting launch must not run") },
        no_notifier,
    )
    .await
    .unwrap();

    assert_eq!(launched, Launch::AlreadyRunning(existing));
    assert_eq!(job_count(&pool).await, 1, "no second job may be created");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn launch_ignores_terminal_jobs_and_other_libraries(pool: PgPool) {
    let lib = create_library(&pool, "launch_free").await;
    let other = create_library(&pool, "launch_other").await;
    create_job(&pool, Some(lib), "metadata_batch", "success").await;
    create_job(&pool, Some(other), "metadata_batch", "running").await;

    let launched = launch_with(
        &pool,
        Some(lib),
        &SPEC,
        |_p, _j| async { Ok(()) },
        no_notifier,
    )
    .await
    .unwrap();

    assert!(matches!(launched, Launch::Started(_)));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn failed_run_fails_the_job_with_partial_stats_and_notifies(pool: PgPool) {
    let lib = create_library(&pool, "Failing Library").await;
    let (notify, mut events) = channel_notifier();

    let launched = launch_with(
        &pool,
        Some(lib),
        &SPEC,
        |_pool, _job_id| async { Err("provider exploded".to_string()) },
        notify,
    )
    .await
    .unwrap();
    let Launch::Started(job_id) = launched else {
        panic!("expected a started job");
    };

    // The notification is sent after `fail_job`, so the row is final once it arrives.
    let event = tokio::time::timeout(TIMEOUT, events.recv())
        .await
        .unwrap()
        .unwrap();
    match event {
        NotificationEvent::MetadataBatchFailed {
            library_name,
            error,
        } => {
            assert_eq!(library_name.as_deref(), Some("Failing Library"));
            assert_eq!(error, "provider exploded");
        }
        _ => panic!("unexpected notification"),
    }

    let row = sqlx::query("SELECT status, error_opt, stats_json FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(
        row.get::<Option<String>, _>("error_opt").as_deref(),
        Some("provider exploded")
    );
    let stats: serde_json::Value = row.get("stats_json");
    assert_eq!(stats["processed"], 3);
    assert_eq!(stats["job"], job_id.to_string());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn failure_does_not_overwrite_a_cancelled_job(pool: PgPool) {
    let lib = create_library(&pool, "launch_cancelled").await;
    let (notify, mut events) = channel_notifier();

    let launched = launch_with(
        &pool,
        Some(lib),
        &SPEC,
        |pool, job_id| async move {
            crate::jobs::lifecycle::cancel_job(&pool, job_id)
                .await
                .unwrap();
            Err("stopped".to_string())
        },
        notify,
    )
    .await
    .unwrap();
    let Launch::Started(job_id) = launched else {
        panic!("expected a started job");
    };
    tokio::time::timeout(TIMEOUT, events.recv())
        .await
        .unwrap()
        .unwrap();

    let status: String = sqlx::query_scalar("SELECT status FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "cancelled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn launch_each_skips_libraries_with_a_conflicting_job(pool: PgPool) {
    let free = create_library(&pool, "fan_free").await;
    let busy = create_library(&pool, "fan_busy").await;
    let also_free = create_library(&pool, "fan_also_free").await;
    // The global mode checks the same conflicts as the per-library mode.
    let existing = create_job(&pool, Some(busy), "metadata_batch_rematch", "running").await;
    let (ran_tx, mut ran_rx) = mpsc::unbounded_channel();

    let fan_out = launch_each_with(
        &pool,
        vec![free, busy, also_free],
        &SPEC,
        move |_pool, job_id, library_id| {
            let ran_tx = ran_tx.clone();
            async move {
                ran_tx.send((job_id, library_id)).unwrap();
                Ok(())
            }
        },
        no_notifier,
    )
    .await
    .unwrap();

    assert_eq!(fan_out.started.len(), 2);
    assert_eq!(fan_out.already_running, vec![existing]);

    let mut ran = Vec::new();
    for _ in 0..2 {
        ran.push(
            tokio::time::timeout(TIMEOUT, ran_rx.recv())
                .await
                .unwrap()
                .unwrap(),
        );
    }
    ran.sort();
    let mut expected: Vec<(Uuid, Uuid)> = Vec::new();
    for job_id in &fan_out.started {
        let library_id: Uuid =
            sqlx::query_scalar("SELECT library_id FROM index_jobs WHERE id = $1")
                .bind(job_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        expected.push((*job_id, library_id));
    }
    expected.sort();
    assert_eq!(ran, expected, "each job runs with its own library");
    let launched_libraries: Vec<Uuid> = expected.iter().map(|(_, lib)| *lib).collect();
    assert!(launched_libraries.contains(&free));
    assert!(launched_libraries.contains(&also_free));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn global_launch_is_scoped_to_global_jobs(pool: PgPool) {
    let lib = create_library(&pool, "global_scope").await;
    // A per-library job does not block a global one…
    create_job(&pool, Some(lib), "prowlarr_rss", "running").await;

    let launched = launch_with(
        &pool,
        None,
        &GLOBAL_SPEC,
        |_p, _j| async { Ok(()) },
        no_notifier,
    )
    .await
    .unwrap();
    let Launch::Started(job_id) = launched else {
        panic!("expected a started global job");
    };
    let library_id: Option<Uuid> =
        sqlx::query_scalar("SELECT library_id FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(library_id, None);

    // …but a global job does.
    let again = launch_with(
        &pool,
        None,
        &GLOBAL_SPEC,
        |_p, _j| async { Ok(()) },
        no_notifier,
    )
    .await
    .unwrap();
    assert_eq!(again, Launch::AlreadyRunning(job_id));
}

#[test]
fn response_keeps_the_backoffice_json_shape() {
    let id = Uuid::new_v4();

    let started = serde_json::to_value(JobLaunchResponse::from(Launch::Started(id))).unwrap();
    assert_eq!(
        started,
        serde_json::json!({ "id": id.to_string(), "status": "running" })
    );

    let running =
        serde_json::to_value(JobLaunchResponse::from(Launch::AlreadyRunning(id))).unwrap();
    assert_eq!(
        running,
        serde_json::json!({ "id": id.to_string(), "status": "already_running" })
    );

    let empty = serde_json::to_value(JobLaunchResponse::from(FanOut::default())).unwrap();
    assert_eq!(
        empty,
        serde_json::json!({ "id": null, "status": "started" })
    );

    let last = Uuid::new_v4();
    let fan_out = JobLaunchResponse::from(FanOut {
        started: vec![id, last],
        already_running: vec![],
    });
    assert_eq!(fan_out.id, Some(last));
    assert_eq!(fan_out.status, JobLaunchStatus::Started);
}

#[test]
fn blocking_types_include_the_job_type_and_its_conflicts() {
    assert_eq!(
        SPEC.blocking_types(),
        vec!["metadata_batch", "metadata_batch_rematch"]
    );
    assert_eq!(
        crate::metadata::batch::METADATA_BATCH_REMATCH.blocking_types(),
        vec!["metadata_batch_rematch", "metadata_batch"]
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn concurrent_launches_start_a_single_job(pool: PgPool) {
    let library = create_library(&pool, "race").await;

    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            launch_with(
                &pool,
                Some(library),
                &SPEC,
                |_pool, _job_id| async {
                    // Keep the job in flight for the whole test.
                    std::future::pending::<()>().await;
                    Ok(())
                },
                no_notifier,
            )
            .await
            .unwrap()
        }));
    }

    let mut started = 0;
    for handle in handles {
        if matches!(handle.await.unwrap(), Launch::Started(_)) {
            started += 1;
        }
    }
    assert_eq!(started, 1, "exactly one launch must win");
    assert_eq!(job_count(&pool).await, 1);
}
