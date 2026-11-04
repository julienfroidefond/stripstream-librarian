use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::jobs::lifecycle::{
    classify_job, complete_job, fail_job, finish_job, job_in_flight, JobScope, JobStatus,
};

async fn create_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
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

async fn insert_job(
    pool: &sqlx::PgPool,
    library_id: Option<Uuid>,
    job_type: &str,
    status: &str,
) -> Uuid {
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

async fn job_row(pool: &sqlx::PgPool, job_id: Uuid) -> sqlx::postgres::PgRow {
    sqlx::query(
        "SELECT status, error_opt, stats_json, progress_percent, finished_at \
         FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

// -- classify_job ----------------------------------------------------------

#[test]
fn classify_job_without_errors_is_success() {
    assert_eq!(classify_job(10, 0), JobStatus::Success);
    assert_eq!(classify_job(0, 0), JobStatus::Success);
}

#[test]
fn classify_job_with_some_errors_is_partial() {
    assert_eq!(classify_job(9, 1), JobStatus::Partial);
    assert_eq!(classify_job(4, 3), JobStatus::Partial);
}

#[test]
fn classify_job_when_every_attempt_failed_is_failed() {
    assert_eq!(classify_job(9, 9), JobStatus::Failed);
    assert_eq!(classify_job(3, 4), JobStatus::Failed);
}

#[test]
fn classify_job_with_errors_and_nothing_attempted_is_failed() {
    assert_eq!(classify_job(0, 1), JobStatus::Failed);
}

// -- job_in_flight ---------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn in_flight_matches_library_scope(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "lib-a").await;
    let other = create_library(&pool, "lib-b").await;
    let wanted = insert_job(&pool, Some(lib), "metadata_batch", "pending").await;
    insert_job(&pool, Some(other), "metadata_batch", "pending").await;

    let found = job_in_flight(&pool, JobScope::Library(lib), &["metadata_batch"])
        .await
        .unwrap();
    assert_eq!(found, Some(wanted));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn in_flight_ignores_finished_and_other_types(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "lib-a").await;
    insert_job(&pool, Some(lib), "metadata_batch", "success").await;
    insert_job(&pool, Some(lib), "metadata_batch", "failed").await;
    insert_job(&pool, Some(lib), "download_detection", "running").await;

    let found = job_in_flight(&pool, JobScope::Library(lib), &["metadata_batch"])
        .await
        .unwrap();
    assert!(found.is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn in_flight_matches_multiple_types(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "lib-a").await;
    let wanted = insert_job(&pool, Some(lib), "metadata_batch_rematch", "running").await;

    let found = job_in_flight(
        &pool,
        JobScope::Library(lib),
        &["metadata_batch", "metadata_batch_rematch"],
    )
    .await
    .unwrap();
    assert_eq!(found, Some(wanted));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn in_flight_global_only_matches_null_library(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "lib-a").await;
    // A library-scoped job must not be matched by a Global lookup...
    insert_job(&pool, Some(lib), "prowlarr_rss", "running").await;
    assert!(job_in_flight(&pool, JobScope::Global, &["prowlarr_rss"])
        .await
        .unwrap()
        .is_none());

    // ...but a truly global one is.
    let global = insert_job(&pool, None, "prowlarr_rss", "pending").await;
    assert_eq!(
        job_in_flight(&pool, JobScope::Global, &["prowlarr_rss"])
            .await
            .unwrap(),
        Some(global)
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn in_flight_any_ignores_library(pool: sqlx::PgPool) {
    let lib = create_library(&pool, "lib-a").await;
    insert_job(&pool, Some(lib), "rating_pull", "running").await;
    insert_job(&pool, None, "rating_pull", "pending").await;

    // Either row is acceptable; the helper only guarantees a match exists.
    let found = job_in_flight(&pool, JobScope::Any, &["rating_pull"])
        .await
        .unwrap();
    assert!(found.is_some());
}

// -- fail_job --------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn fail_job_without_stats_sets_status_and_error(pool: sqlx::PgPool) {
    let job = insert_job(&pool, None, "scan", "running").await;

    fail_job(&pool, job, "boom", None).await.unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(
        row.get::<Option<String>, _>("error_opt").as_deref(),
        Some("boom")
    );
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("finished_at")
        .is_some());
    assert!(row
        .get::<Option<serde_json::Value>, _>("stats_json")
        .is_none());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn fail_job_with_stats_stores_payload(pool: sqlx::PgPool) {
    let job = insert_job(&pool, None, "scan", "running").await;

    fail_job(&pool, job, "boom", Some(json!({ "processed": 3 })))
        .await
        .unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(
        row.get::<serde_json::Value, _>("stats_json"),
        json!({ "processed": 3 })
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn fail_job_without_stats_preserves_existing_stats(pool: sqlx::PgPool) {
    let job = insert_job(&pool, None, "scan", "running").await;
    sqlx::query("UPDATE index_jobs SET stats_json = $2 WHERE id = $1")
        .bind(job)
        .bind(json!({ "kept": true }))
        .execute(&pool)
        .await
        .unwrap();

    fail_job(&pool, job, "boom", None).await.unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(
        row.get::<serde_json::Value, _>("stats_json"),
        json!({ "kept": true })
    );
}

// -- complete_job ----------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn complete_job_sets_success_and_progress(pool: sqlx::PgPool) {
    let job = insert_job(&pool, None, "scan", "running").await;

    complete_job(&pool, job, json!({ "total": 10 }))
        .await
        .unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "success");
    assert_eq!(row.get::<i32, _>("progress_percent"), 100);
    assert_eq!(
        row.get::<serde_json::Value, _>("stats_json"),
        json!({ "total": 10 })
    );
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("finished_at")
        .is_some());
}

// -- finish_job ------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn finish_job_partial_stores_summary_and_completes_progress(pool: sqlx::PgPool) {
    let job = insert_job(&pool, None, "prowlarr_rss", "running").await;

    finish_job(
        &pool,
        job,
        JobStatus::Partial,
        json!({ "errors": 4 }),
        Some("4/6 indexer(s) en erreur"),
    )
    .await
    .unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "partial");
    assert_eq!(row.get::<i32, _>("progress_percent"), 100);
    assert_eq!(
        row.get::<Option<String>, _>("error_opt").as_deref(),
        Some("4/6 indexer(s) en erreur")
    );
    assert_eq!(
        row.get::<serde_json::Value, _>("stats_json"),
        json!({ "errors": 4 })
    );
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("finished_at")
        .is_some());
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn finish_job_failed_keeps_current_progress(pool: sqlx::PgPool) {
    let job = insert_job(&pool, None, "ddl_search", "running").await;
    sqlx::query("UPDATE index_jobs SET progress_percent = 42 WHERE id = $1")
        .bind(job)
        .execute(&pool)
        .await
        .unwrap();

    finish_job(
        &pool,
        job,
        JobStatus::Failed,
        json!({ "errors": 81, "files": 0 }),
        Some("81 provider search(es) failed"),
    )
    .await
    .unwrap();

    let row = job_row(&pool, job).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(row.get::<i32, _>("progress_percent"), 42);
    assert_eq!(
        row.get::<Option<String>, _>("error_opt").as_deref(),
        Some("81 provider search(es) failed")
    );
}

// -- status constraint -----------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn status_check_accepts_partial(pool: sqlx::PgPool) {
    // The migration must widen `index_jobs_status_check` so the new terminal
    // status is a legal value, not just a Rust enum variant.
    let job = insert_job(&pool, None, "prowlarr_rss", "partial").await;
    assert_eq!(
        job_row(&pool, job).await.get::<String, _>("status"),
        "partial"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn status_check_still_rejects_unknown_statuses(pool: sqlx::PgPool) {
    let attempted = sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'scan', 'partially_done')",
    )
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await;
    assert!(attempted.is_err());
}
