//! Progress, cancellation and event helpers shared by API and indexer jobs.

use sqlx::PgPool;
use uuid::Uuid;

/// Check if a job has been cancelled. A missing job is treated as not cancelled.
pub async fn is_job_cancelled(pool: &PgPool, job_id: Uuid) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT status = 'cancelled' FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false)
}

/// Update job progress and ignore failures (progress is best-effort).
pub async fn update_progress(
    pool: &PgPool,
    job_id: Uuid,
    processed: i32,
    total: i32,
    current: &str,
) {
    let _ = update_progress_checked(pool, job_id, processed, total, current).await;
}

/// Update job progress, returning database errors to the caller.
pub async fn update_progress_checked(
    pool: &PgPool,
    job_id: Uuid,
    processed: i32,
    total: i32,
    current: &str,
) -> Result<(), sqlx::Error> {
    // Integer math: `(processed as f64 / total as f64 * 100.0) as i32` truncates
    // to 28 for 29/100 because of binary rounding, so keep the division exact.
    let percent = if total > 0 {
        ((processed.max(0) as i64 * 100) / total as i64).clamp(0, 100) as i32
    } else {
        0
    };

    sqlx::query(
        "UPDATE index_jobs SET processed_files = $2, progress_percent = $3, current_file = $4 WHERE id = $1",
    )
    .bind(job_id)
    .bind(processed)
    .bind(percent)
    .bind(current)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Set `processed_files` and `progress_percent` without a current item name.
pub async fn set_progress(
    pool: &PgPool,
    job_id: Uuid,
    processed: i32,
    percent: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1")
        .bind(job_id)
        .bind(processed)
        .bind(percent.clamp(0, 100))
        .execute(pool)
        .await
        .map(|_| ())
}

/// Set the file currently being processed.
pub async fn set_current_file(
    pool: &PgPool,
    job_id: Uuid,
    current: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE index_jobs SET current_file = $2 WHERE id = $1")
        .bind(job_id)
        .bind(current)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Set the progress denominator.
pub async fn set_total_files(pool: &PgPool, job_id: Uuid, total: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Insert an event into the unified job events table (best-effort).
#[allow(clippy::too_many_arguments)]
pub async fn insert_event(
    pool: &PgPool,
    job_id: Uuid,
    event_type: &str,
    level: &str,
    entity_type: Option<&str>,
    entity_name: Option<&str>,
    message: Option<&str>,
    detail: Option<serde_json::Value>,
) {
    let _ = sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name, message, detail) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(job_id)
    .bind(event_type)
    .bind(level)
    .bind(entity_type)
    .bind(entity_name)
    .bind(message)
    .bind(detail)
    .execute(pool)
    .await;
}

/// Convenience wrapper for events that carry only an entity *name* (no
/// `entity_type`), the shape used by the download and reading-status jobs.
#[allow(clippy::too_many_arguments)]
pub async fn insert_entity_event(
    pool: &PgPool,
    job_id: Uuid,
    event_type: &str,
    level: &str,
    entity_name: Option<&str>,
    message: Option<&str>,
    detail: Option<serde_json::Value>,
) {
    insert_event(
        pool,
        job_id,
        event_type,
        level,
        None,
        entity_name,
        message,
        detail,
    )
    .await;
}
