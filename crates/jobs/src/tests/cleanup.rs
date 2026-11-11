use std::time::Duration;

use sqlx::PgPool;

use crate::cleanup::{cleanup_stale_jobs, fail_orphaned_jobs, purge_terminal_jobs};
use crate::registry::JobOwner;
use crate::tests::support::{age_created_at, age_started_at, create_library, insert_job, status};
use crate::JobState;

#[sqlx::test(migrations = "../../infra/migrations")]
async fn fail_orphaned_jobs_is_owner_scoped_and_covers_every_active_state(pool: PgPool) {
    let library = create_library(&pool, "lib").await;

    let mut api_jobs = Vec::new();
    for state in JobState::ACTIVE {
        api_jobs
            .push(insert_job(&pool, Some(library), "reading_status_match", state.as_str()).await);
    }
    let indexer = insert_job(&pool, Some(library), "scan", "running").await;

    let changed = fail_orphaned_jobs(&pool, JobOwner::Api, "interrupted by API restart")
        .await
        .unwrap();

    assert_eq!(changed, api_jobs.len() as u64);
    for job in api_jobs {
        assert_eq!(status(&pool, job).await, "failed");
    }
    assert_eq!(status(&pool, indexer).await, "running");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn cleanup_stale_active_jobs_is_owner_scoped(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let stale = insert_job(&pool, Some(library), "scan", "running").await;
    age_started_at(&pool, stale, 600).await;
    let fresh = insert_job(
        &pool,
        Some(library),
        "thumbnail_rebuild",
        "generating_thumbnails",
    )
    .await;
    age_started_at(&pool, fresh, 10).await;
    let api = insert_job(&pool, Some(library), "metadata_batch", "running").await;
    age_started_at(&pool, api, 600).await;

    let report = cleanup_stale_jobs(
        &pool,
        JobOwner::Indexer,
        Some(Duration::from_secs(300)),
        None,
        "interrupted",
        "stale",
    )
    .await
    .unwrap();

    assert_eq!(report.active, 1);
    assert_eq!(report.pending, 0);
    assert_eq!(status(&pool, stale).await, "failed");
    assert_eq!(status(&pool, fresh).await, "generating_thumbnails");
    assert_eq!(status(&pool, api).await, "running");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn cleanup_stale_pending_jobs_is_owner_scoped(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let stale = insert_job(&pool, Some(library), "scan", "pending").await;
    age_created_at(&pool, stale, 3600).await;
    let fresh = insert_job(&pool, Some(library), "rebuild", "pending").await;
    age_created_at(&pool, fresh, 10).await;
    let api = insert_job(&pool, Some(library), "metadata_batch", "pending").await;
    age_created_at(&pool, api, 3600).await;

    let report = cleanup_stale_jobs(
        &pool,
        JobOwner::Indexer,
        None,
        Some(Duration::from_secs(1800)),
        "active",
        "never picked up",
    )
    .await
    .unwrap();

    assert_eq!(report.active, 0);
    assert_eq!(report.pending, 1);
    assert_eq!(status(&pool, stale).await, "failed");
    assert_eq!(status(&pool, fresh).await, "pending");
    assert_eq!(status(&pool, api).await, "pending");

    let error: Option<String> =
        sqlx::query_scalar("SELECT error_opt FROM index_jobs WHERE id = $1")
            .bind(stale)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(error.as_deref(), Some("never picked up"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn cleanup_with_no_thresholds_changes_nothing(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let pending = insert_job(&pool, Some(library), "scan", "pending").await;
    age_created_at(&pool, pending, 3600).await;
    let running = insert_job(&pool, Some(library), "scan", "running").await;
    age_started_at(&pool, running, 3600).await;

    let report = cleanup_stale_jobs(&pool, JobOwner::Indexer, None, None, "a", "b")
        .await
        .unwrap();

    assert_eq!(report, Default::default());
    assert_eq!(status(&pool, pending).await, "pending");
    assert_eq!(status(&pool, running).await, "running");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn cleanup_ignores_pending_jobs_without_started_at(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let job = insert_job(&pool, Some(library), "scan", "running").await;

    let report = cleanup_stale_jobs(
        &pool,
        JobOwner::Indexer,
        Some(Duration::from_secs(1)),
        None,
        "a",
        "b",
    )
    .await
    .unwrap();

    assert_eq!(report.active, 0);
    assert_eq!(status(&pool, job).await, "running");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn purge_terminal_jobs_only_deletes_old_terminal_rows(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let old_success = insert_job(&pool, Some(library), "scan", "success").await;
    age_created_at(&pool, old_success, 91 * 24 * 3600).await;
    let old_failed = insert_job(&pool, Some(library), "scan", "failed").await;
    age_created_at(&pool, old_failed, 91 * 24 * 3600).await;
    let recent = insert_job(&pool, Some(library), "scan", "success").await;
    let old_running = insert_job(&pool, Some(library), "scan", "running").await;
    age_created_at(&pool, old_running, 91 * 24 * 3600).await;

    let purged = purge_terminal_jobs(&pool, 90).await.unwrap();

    assert_eq!(purged, 2);
    assert_eq!(status(&pool, recent).await, "success");
    assert_eq!(status(&pool, old_running).await, "running");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 2);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn legacy_cleanup_only_touches_legacy_types(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let legacy = insert_job(&pool, Some(library), "torrent_import", "running").await;
    age_started_at(&pool, legacy, 3600).await;
    let indexer = insert_job(&pool, Some(library), "scan", "running").await;
    age_started_at(&pool, indexer, 3600).await;

    let report = cleanup_stale_jobs(
        &pool,
        JobOwner::Legacy,
        Some(Duration::from_secs(1)),
        Some(Duration::from_secs(1)),
        "a",
        "b",
    )
    .await
    .unwrap();

    assert_eq!(report.active, 1);
    assert_eq!(status(&pool, legacy).await, "failed");
    assert_eq!(status(&pool, indexer).await, "running");
}
