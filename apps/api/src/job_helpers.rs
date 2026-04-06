//! Shared helpers for background job processing.
//! Used by metadata_batch, metadata_refresh, download_detection, reading_status_*.

use sqlx::PgPool;
use uuid::Uuid;

/// Check if a job has been cancelled.
pub(crate) async fn is_job_cancelled(pool: &PgPool, job_id: Uuid) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT status = 'cancelled' FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

/// Update job progress (processed count, percentage, current item name).
pub(crate) async fn update_progress(
    pool: &PgPool,
    job_id: Uuid,
    processed: i32,
    total: i32,
    current: &str,
) {
    let percent = if total > 0 {
        (processed as f64 / total as f64 * 100.0) as i32
    } else {
        0
    };

    let _ = sqlx::query(
        "UPDATE index_jobs SET processed_files = $2, progress_percent = $3, current_file = $4 WHERE id = $1",
    )
    .bind(job_id)
    .bind(processed)
    .bind(percent)
    .bind(current)
    .execute(pool)
    .await;
}

/// Insert an event into the unified job events table.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_event(
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
