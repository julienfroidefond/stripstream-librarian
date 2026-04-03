use std::time::Duration;

use sqlx::{PgPool, Row};
use tracing::{error, info, trace};
use uuid::Uuid;

use crate::{download_detection, metadata_batch, metadata_refresh, reading_status_push};

/// Poll for pending API-only jobs (`metadata_batch`, `metadata_refresh`) and process them.
/// This mirrors the indexer's worker loop but for job types handled by the API.
pub async fn run_job_poller(pool: PgPool, interval_seconds: u64) {
    let wait = Duration::from_secs(interval_seconds.max(1));

    loop {
        match claim_next_api_job(&pool).await {
            Ok(Some((job_id, job_type, library_id))) => {
                info!("[JOB_POLLER] Claimed {job_type} job {job_id} library={library_id}");

                let pool_clone = pool.clone();
                let library_name: Option<String> =
                    sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
                        .bind(library_id)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten();

                tokio::spawn(async move {
                    let result = match job_type.as_str() {
                        "metadata_refresh" => {
                            metadata_refresh::process_metadata_refresh(
                                &pool_clone,
                                job_id,
                                library_id,
                            )
                            .await
                        }
                        "metadata_refresh_all" => {
                            metadata_refresh::process_metadata_refresh_all(
                                &pool_clone,
                                job_id,
                                library_id,
                            )
                            .await
                        }
                        "metadata_batch" | "metadata_batch_rematch" => {
                            metadata_batch::process_metadata_batch(
                                &pool_clone,
                                job_id,
                                library_id,
                            )
                            .await
                        }
                        "reading_status_push" => {
                            reading_status_push::process_reading_status_push(
                                &pool_clone,
                                job_id,
                                library_id,
                            )
                            .await
                        }
                        "download_detection" => {
                            download_detection::process_download_detection(
                                &pool_clone,
                                job_id,
                                library_id,
                            )
                            .await
                            .map(|_| ())
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
                            "download_detection" => {
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

const API_JOB_TYPES: &[&str] = &["metadata_batch", "metadata_batch_rematch", "metadata_refresh", "metadata_refresh_all", "reading_status_push", "download_detection"];

async fn claim_next_api_job(pool: &PgPool) -> Result<Option<(Uuid, String, Uuid)>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query(
        r#"
        SELECT id, type, library_id
        FROM index_jobs
        WHERE status = 'pending'
          AND type = ANY($1)
          AND library_id IS NOT NULL
        ORDER BY created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT 1
        "#,
    )
    .bind(API_JOB_TYPES)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let id: Uuid = row.get("id");
    let job_type: String = row.get("type");
    let library_id: Uuid = row.get("library_id");

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
mod tests {
    use super::*;

    #[test]
    fn api_job_types_contains_metadata_refresh_all() {
        assert!(
            API_JOB_TYPES.contains(&"metadata_refresh_all"),
            "API_JOB_TYPES must include metadata_refresh_all"
        );
    }

    #[test]
    fn api_job_types_contains_all_expected_types() {
        let expected = &[
            "metadata_batch",
            "metadata_refresh",
            "metadata_refresh_all",
            "reading_status_push",
            "download_detection",
        ];
        for t in expected {
            assert!(
                API_JOB_TYPES.contains(t),
                "API_JOB_TYPES is missing: {t}"
            );
        }
    }
}
