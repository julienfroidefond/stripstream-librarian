use serde_json::json;
use sqlx::PgPool;
use sqlx::Row;
use uuid::Uuid;

use crate::helpers::{
    insert_entity_event, insert_event, is_job_cancelled, set_current_file, set_progress,
    set_total_files, update_progress, update_progress_checked,
};

async fn seed_job(pool: &PgPool) -> Uuid {
    let job_id = Uuid::new_v4();
    sqlx::query("INSERT INTO index_jobs (id, type, status) VALUES ($1, 'rating_pull', 'running')")
        .bind(job_id)
        .execute(pool)
        .await
        .unwrap();
    job_id
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_progress_writes_count_percent_and_current(pool: PgPool) {
    let job_id = seed_job(&pool).await;

    update_progress(&pool, job_id, 29, 100, "Astérix").await;

    let row = sqlx::query(
        "SELECT processed_files, progress_percent, current_file FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i32, _>("processed_files"), 29);
    assert_eq!(row.get::<i32, _>("progress_percent"), 29);
    assert_eq!(row.get::<String, _>("current_file"), "Astérix");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn progress_helpers_write_independent_fields(pool: PgPool) {
    let job_id = seed_job(&pool).await;

    set_total_files(&pool, job_id, 50).await.unwrap();
    set_progress(&pool, job_id, 10, 20).await.unwrap();
    set_current_file(&pool, job_id, "Tintin").await.unwrap();

    let row = sqlx::query(
        "SELECT total_files, processed_files, progress_percent, current_file \
         FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i32, _>("total_files"), 50);
    assert_eq!(row.get::<i32, _>("processed_files"), 10);
    assert_eq!(row.get::<i32, _>("progress_percent"), 20);
    assert_eq!(row.get::<String, _>("current_file"), "Tintin");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_progress_is_zero_percent_without_total(pool: PgPool) {
    let job_id = seed_job(&pool).await;

    update_progress(&pool, job_id, 0, 0, "Astérix").await;

    let percent: i32 = sqlx::query_scalar("SELECT progress_percent FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(percent, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn update_progress_checked_ignores_missing_jobs(pool: PgPool) {
    let missing = Uuid::new_v4();
    update_progress_checked(&pool, missing, 1, 2, "x")
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs WHERE id = $1")
        .bind(missing)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn is_job_cancelled_reflects_status(pool: PgPool) {
    let job_id = seed_job(&pool).await;
    assert!(!is_job_cancelled(&pool, job_id).await);

    sqlx::query("UPDATE index_jobs SET status = 'cancelled' WHERE id = $1")
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();

    assert!(is_job_cancelled(&pool, job_id).await);
    assert!(!is_job_cancelled(&pool, Uuid::new_v4()).await);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn event_helpers_store_events(pool: PgPool) {
    let job_id = seed_job(&pool).await;

    insert_event(
        &pool,
        job_id,
        "series_matched",
        "info",
        Some("series"),
        Some("Astérix"),
        Some("matched"),
        Some(json!({"provider": "bdtheque"})),
    )
    .await;
    insert_entity_event(
        &pool,
        job_id,
        "download_found",
        "info",
        Some("Tintin"),
        Some("release found"),
        None,
    )
    .await;

    let rows = sqlx::query(
        "SELECT event_type, entity_type, entity_name, message, detail \
         FROM index_job_events WHERE job_id = $1 ORDER BY created_at",
    )
    .bind(job_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("event_type"), "series_matched");
    assert_eq!(
        rows[0].get::<Option<String>, _>("entity_type").as_deref(),
        Some("series")
    );
    assert_eq!(
        rows[0].get::<Option<serde_json::Value>, _>("detail"),
        Some(json!({"provider": "bdtheque"}))
    );
    assert_eq!(rows[1].get::<Option<String>, _>("entity_type"), None);
    assert_eq!(
        rows[1].get::<Option<String>, _>("entity_name").as_deref(),
        Some("Tintin")
    );
}
