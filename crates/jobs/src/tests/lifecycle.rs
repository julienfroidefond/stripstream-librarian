use serde_json::json;
use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

use crate::lifecycle::{
    begin_analysis, begin_extraction, begin_thumbnail_generation, begin_thumbnail_job, cancel_job,
    classify_job, complete_job, fail_job, finish_job, finish_success, job_in_flight, start_job,
    JobScope, JobStatus,
};
use crate::tests::support::{create_library, insert_job, job_row, status};
use crate::JobState;

#[test]
fn classification_maps_outcomes_to_terminal_statuses() {
    assert_eq!(classify_job(10, 0), JobStatus::Success);
    assert_eq!(classify_job(10, 3), JobStatus::Partial);
    assert_eq!(classify_job(10, 10), JobStatus::Failed);
    assert_eq!(classify_job(10, 11), JobStatus::Failed);
    assert_eq!(classify_job(0, 1), JobStatus::Failed);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn job_in_flight_covers_all_active_states(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    assert!(job_in_flight(&pool, JobScope::Any, &["scan"])
        .await
        .unwrap()
        .is_none());

    for state in [
        JobState::Pending,
        JobState::Running,
        JobState::ExtractingPages,
        JobState::GeneratingThumbnails,
    ] {
        let job = insert_job(&pool, Some(library), "scan", state.as_str()).await;
        assert_eq!(
            job_in_flight(&pool, JobScope::Any, &["scan"])
                .await
                .unwrap(),
            Some(job)
        );
        sqlx::query("UPDATE index_jobs SET status = 'success' WHERE id = $1")
            .bind(job)
            .execute(&pool)
            .await
            .unwrap();
    }

    for state in [
        JobState::Success,
        JobState::Partial,
        JobState::Failed,
        JobState::Cancelled,
    ] {
        let job = insert_job(&pool, Some(library), "scan", state.as_str()).await;
        assert!(
            job_in_flight(&pool, JobScope::Any, &["scan"])
                .await
                .unwrap()
                .is_none(),
            "{} must not count as in flight",
            state.as_str()
        );
        sqlx::query("DELETE FROM index_jobs WHERE id = $1")
            .bind(job)
            .execute(&pool)
            .await
            .unwrap();
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn job_in_flight_respects_scope(pool: PgPool) {
    let first = create_library(&pool, "first").await;
    let second = create_library(&pool, "second").await;

    let library_job = insert_job(&pool, Some(first), "metadata_batch", "running").await;
    let global_job = insert_job(&pool, None, "prowlarr_rss", "running").await;

    assert_eq!(
        job_in_flight(&pool, JobScope::Library(first), &["metadata_batch"])
            .await
            .unwrap(),
        Some(library_job)
    );
    assert!(
        job_in_flight(&pool, JobScope::Library(second), &["metadata_batch"])
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        job_in_flight(&pool, JobScope::Global, &["prowlarr_rss"])
            .await
            .unwrap(),
        Some(global_job)
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn fail_job_marks_failed_and_merges_stats(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "running").await;

    fail_job(&pool, job, "boom", Some(json!({"errors": 2})))
        .await
        .unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(row.get::<String, _>("error_opt"), "boom");
    assert_eq!(
        row.get::<serde_json::Value, _>("stats_json"),
        json!({"errors": 2})
    );
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("finished_at")
        .is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn fail_job_does_not_overwrite_cancellation(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "cancelled").await;

    fail_job(&pool, job, "late failure", None).await.unwrap();

    assert_eq!(status(&pool, job).await, "cancelled");
    let error: Option<String> =
        sqlx::query_scalar("SELECT error_opt FROM index_jobs WHERE id = $1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(error.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn finish_job_writes_terminal_status_and_stats(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "running").await;

    finish_job(
        &pool,
        job,
        JobStatus::Partial,
        json!({"ok": 1}),
        Some("1 failed"),
    )
    .await
    .unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "partial");
    assert_eq!(row.get::<String, _>("error_opt"), "1 failed");
    assert_eq!(row.get::<i32, _>("progress_percent"), 100);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn finish_job_keeps_progress_for_failures(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "running").await;
    sqlx::query("UPDATE index_jobs SET progress_percent = 42 WHERE id = $1")
        .bind(job)
        .execute(&pool)
        .await
        .unwrap();

    finish_job(&pool, job, JobStatus::Failed, json!({}), Some("boom"))
        .await
        .unwrap();

    assert_eq!(
        job_row(&pool, job).await.get::<i32, _>("progress_percent"),
        42
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn finish_job_does_not_overwrite_cancellation(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "cancelled").await;

    finish_job(&pool, job, JobStatus::Success, json!({}), None)
        .await
        .unwrap();

    assert_eq!(status(&pool, job).await, "cancelled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn complete_job_marks_success(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "running").await;

    complete_job(&pool, job, json!({"done": true}))
        .await
        .unwrap();

    assert_eq!(status(&pool, job).await, "success");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn start_job_sets_running_and_started_at(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "pending").await;

    start_job(&pool, job).await.unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "running");
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("started_at")
        .is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn start_job_does_not_resurrect_cancelled_jobs(pool: PgPool) {
    let job = insert_job(&pool, None, "metadata_batch", "cancelled").await;

    start_job(&pool, job).await.unwrap();

    assert_eq!(status(&pool, job).await, "cancelled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn cancel_job_only_transitions_in_flight_jobs(pool: PgPool) {
    for state in [
        JobState::Pending,
        JobState::Running,
        JobState::ExtractingPages,
        JobState::GeneratingThumbnails,
    ] {
        let job = insert_job(&pool, None, "scan", state.as_str()).await;
        assert_eq!(cancel_job(&pool, job).await.unwrap(), 1);
        assert_eq!(status(&pool, job).await, "cancelled");
    }

    let finished = insert_job(&pool, None, "scan", "success").await;
    assert_eq!(cancel_job(&pool, finished).await.unwrap(), 0);
    assert_eq!(status(&pool, finished).await, "success");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn begin_thumbnail_job_sets_both_timestamps(pool: PgPool) {
    let job = insert_job(&pool, None, "thumbnail_rebuild", "running").await;

    begin_thumbnail_job(&pool, job).await.unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "generating_thumbnails");
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("started_at")
        .is_some());
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("phase2_started_at")
        .is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn phase_transitions_reset_and_set_progress(pool: PgPool) {
    let job = insert_job(&pool, None, "scan", "running").await;
    sqlx::query("UPDATE index_jobs SET processed_files = 12, current_file = 'old' WHERE id = $1")
        .bind(job)
        .execute(&pool)
        .await
        .unwrap();

    begin_extraction(&pool, job, 100).await.unwrap();
    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "extracting_pages");
    assert_eq!(row.get::<i32, _>("total_files"), 100);
    assert_eq!(row.get::<i32, _>("processed_files"), 0);
    assert!(row.get::<Option<String>, _>("current_file").is_none());

    begin_thumbnail_generation(&pool, job, 80).await.unwrap();
    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "generating_thumbnails");
    assert_eq!(row.get::<i32, _>("total_files"), 80);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn begin_analysis_persists_stats_and_processed_count(pool: PgPool) {
    let job = insert_job(&pool, None, "scan", "extracting_pages").await;

    begin_analysis(&pool, job, json!({"scanned_files": 7}), 7)
        .await
        .unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "generating_thumbnails");
    assert_eq!(row.get::<i32, _>("processed_files"), 7);
    assert_eq!(
        row.get::<serde_json::Value, _>("stats_json"),
        json!({"scanned_files": 7})
    );
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("phase2_started_at")
        .is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn phase_transitions_do_not_resurrect_cancelled_jobs(pool: PgPool) {
    let job = insert_job(&pool, None, "scan", "cancelled").await;

    begin_extraction(&pool, job, 10).await.unwrap();
    begin_thumbnail_generation(&pool, job, 10).await.unwrap();
    begin_analysis(&pool, job, json!({}), 0).await.unwrap();

    assert_eq!(status(&pool, job).await, "cancelled");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn finish_success_clears_current_file(pool: PgPool) {
    let job = insert_job(&pool, None, "scan", "generating_thumbnails").await;
    sqlx::query("UPDATE index_jobs SET current_file = 'book.cbz' WHERE id = $1")
        .bind(job)
        .execute(&pool)
        .await
        .unwrap();

    finish_success(&pool, job).await.unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "success");
    assert_eq!(row.get::<i32, _>("progress_percent"), 100);
    assert!(row.get::<Option<String>, _>("current_file").is_none());
}

#[test]
fn terminal_statuses_are_valid_terminal_states() {
    for status in [JobStatus::Success, JobStatus::Partial, JobStatus::Failed] {
        let parsed: JobState = status.as_str().parse().unwrap();
        assert!(parsed.is_terminal());
    }
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn job_in_flight_accepts_owned_ids_only(pool: PgPool) {
    let library = create_library(&pool, "lib").await;
    let job = insert_job(&pool, Some(library), "scan", "running").await;
    assert_eq!(
        job_in_flight(&pool, JobScope::Library(library), &["scan"])
            .await
            .unwrap(),
        Some(job)
    );
    assert!(
        job_in_flight(&pool, JobScope::Library(Uuid::new_v4()), &["scan"])
            .await
            .unwrap()
            .is_none()
    );
}
