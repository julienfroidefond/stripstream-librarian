use std::time::Duration;

use sqlx::{PgPool, Row};
use tracing::{error, info, trace};
use uuid::Uuid;

use crate::{
    downloads::{detection as download_detection, rss_poll, telegram_monitor},
    metadata, reading,
};

/// Poll for pending API-only jobs (`metadata_batch`, `metadata_refresh`) and process them.
/// This mirrors the indexer's worker loop but for job types handled by the API.
pub async fn run_job_poller(pool: PgPool, interval_seconds: u64) {
    let wait = Duration::from_secs(interval_seconds.max(1));

    loop {
        match claim_next_api_job(&pool).await {
            Ok(Some((job_id, job_type, library_id))) => {
                info!("[JOB_POLLER] Claimed {job_type} job {job_id} library={library_id:?}");

                let pool_clone = pool.clone();
                let library_name: Option<String> = if let Some(lid) = library_id {
                    sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
                        .bind(lid)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten()
                } else {
                    None
                };

                tokio::spawn(async move {
                    let result = match job_type.as_str() {
                        "metadata_refresh" => {
                            metadata::process_metadata_refresh(
                                &pool_clone,
                                job_id,
                                library_id.unwrap(),
                            )
                            .await
                        }
                        "metadata_refresh_all" => {
                            metadata::process_metadata_refresh_all(
                                &pool_clone,
                                job_id,
                                library_id.unwrap(),
                            )
                            .await
                        }
                        "metadata_batch" | "metadata_batch_rematch" => {
                            metadata::process_metadata_batch(
                                &pool_clone,
                                job_id,
                                library_id.unwrap(),
                            )
                            .await
                        }
                        "reading_status_push" => {
                            reading::status_push::process_reading_status_push(
                                &pool_clone,
                                job_id,
                                library_id.unwrap(),
                            )
                            .await
                        }
                        "rating_pull" => {
                            reading::status_pull::process_rating_pull(&pool_clone, job_id).await
                        }
                        "download_detection" => download_detection::process_download_detection(
                            &pool_clone,
                            job_id,
                            library_id.unwrap(),
                        )
                        .await
                        .map(|_| ()),
                        "prowlarr_rss" => {
                            rss_poll::process_rss_poll(&pool_clone, job_id, library_id).await
                        }
                        "telegram_sync" => {
                            telegram_monitor::process_telegram_sync(&pool_clone, job_id).await
                        }
                        "telegram_sync_incremental" => {
                            telegram_monitor::process_telegram_sync_incremental(&pool_clone, job_id)
                                .await
                        }
                        _ => Err(format!("Unknown API job type: {job_type}")),
                    };

                    if let Err(e) = result {
                        error!("[JOB_POLLER] {job_type} job {job_id} failed: {e}");
                        let _ = sqlx::query(
                            "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
                        )
                        .bind(job_id)
                        .bind(e.to_string())
                        .execute(&pool_clone)
                        .await;

                        match job_type.as_str() {
                            "metadata_refresh" | "metadata_refresh_all" => {
                                notifications::notify(
                                    pool_clone,
                                    notifications::NotificationEvent::MetadataRefreshFailed {
                                        library_name,
                                        error: e.to_string(),
                                    },
                                );
                            }
                            "metadata_batch" | "metadata_batch_rematch" => {
                                notifications::notify(
                                    pool_clone,
                                    notifications::NotificationEvent::MetadataBatchFailed {
                                        library_name,
                                        error: e.to_string(),
                                    },
                                );
                            }
                            "reading_status_push" => {
                                notifications::notify(
                                    pool_clone,
                                    notifications::NotificationEvent::ReadingStatusPushFailed {
                                        library_name,
                                        error: e.to_string(),
                                    },
                                );
                            }
                            "download_detection" | "prowlarr_rss" => {
                                notifications::notify(
                                    pool_clone,
                                    notifications::NotificationEvent::DownloadDetectionFailed {
                                        library_name,
                                        error: e.to_string(),
                                    },
                                );
                            }
                            _ => {}
                        }
                    }
                });
            }
            Ok(None) => {
                trace!("[JOB_POLLER] No pending API jobs, waiting...");
                tokio::time::sleep(wait).await;
            }
            Err(err) => {
                error!("[JOB_POLLER] Error claiming job: {err}");
                tokio::time::sleep(wait).await;
            }
        }
    }
}

const API_JOB_TYPES: &[&str] = &[
    "metadata_batch",
    "metadata_batch_rematch",
    "metadata_refresh",
    "metadata_refresh_all",
    "reading_status_push",
    "rating_pull",
    "download_detection",
    "prowlarr_rss",
    "telegram_sync",
    "telegram_sync_incremental",
];

const GLOBAL_METADATA_REFRESH_JOB_TYPES: &[&str] = &["metadata_refresh", "metadata_refresh_all"];
const API_ACTIVE_STATUSES: &[&str] = &["running"];

async fn claim_next_api_job(
    pool: &PgPool,
) -> Result<Option<(Uuid, String, Option<Uuid>)>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let has_active_metadata_refresh: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM index_jobs
            WHERE status = ANY($1)
              AND type = ANY($2)
        )
        "#,
    )
    .bind(API_ACTIVE_STATUSES)
    .bind(GLOBAL_METADATA_REFRESH_JOB_TYPES)
    .fetch_one(&mut *tx)
    .await?;

    let row = sqlx::query(
        r#"
        SELECT id, type, library_id
        FROM index_jobs
        WHERE status = 'pending'
          AND type = ANY($1)
          AND (library_id IS NOT NULL OR type IN ('prowlarr_rss', 'telegram_sync', 'telegram_sync_incremental'))
          AND (
            (type = ANY($2) AND NOT $3::bool)
            OR type != ALL($2)
          )
        ORDER BY created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#,
    )
    .bind(API_JOB_TYPES)
    .bind(GLOBAL_METADATA_REFRESH_JOB_TYPES)
    .bind(has_active_metadata_refresh)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let id: Uuid = row.get("id");
    let job_type: String = row.get("type");
    let library_id: Option<Uuid> = row.try_get("library_id").ok().flatten();

    sqlx::query(
        "UPDATE index_jobs SET status = 'running', started_at = NOW(), error_opt = NULL WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Some((id, job_type, library_id)))
}

#[cfg(test)]
#[path = "tests/poller.rs"]
mod tests;
