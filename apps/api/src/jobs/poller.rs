use std::time::Duration;

use sqlx::PgPool;
use tracing::{error, info, trace};

use stripstream_jobs::{claim_next, fail_job, fail_orphaned_jobs, JobOwner};

use crate::{
    downloads::{ddl_provider, detection as download_detection, rss_poll, telegram_monitor},
    metadata, reading,
};

/// Poll for pending API-owned jobs and process them.
///
/// The set of claimable types, their library requirements and the mutual
/// exclusion rules all come from `stripstream_jobs`, so the API and the indexer
/// cannot drift apart.
pub async fn run_job_poller(pool: PgPool, interval_seconds: u64) {
    let wait = Duration::from_secs(interval_seconds.max(1));

    loop {
        match claim_next(&pool, JobOwner::Api).await {
            Ok(Some(claimed)) => {
                let job_id = claimed.id;
                let job_type = claimed.job_type;
                let library_id = claimed.library_id;
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
                        "ddl_search" => ddl_provider::process_ddl_search(&pool_clone, job_id).await,
                        _ => Err(format!("Unknown API job type: {job_type}")),
                    };

                    if let Err(e) = result {
                        error!("[JOB_POLLER] {job_type} job {job_id} failed: {e}");
                        let _ = fail_job(&pool_clone, job_id, &e, None).await;

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
                            "download_detection" | "prowlarr_rss" | "ddl_search" => {
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

/// Marks API jobs left active by a previous process as failed.
///
/// The API runs its job types in-process, so a restart mid-job would otherwise
/// leave the row active forever. Only API-owned types are touched, leaving
/// indexer-owned jobs intact.
pub async fn fail_orphaned_api_jobs(pool: &PgPool) -> Result<u64, sqlx::Error> {
    fail_orphaned_jobs(pool, JobOwner::Api, "interrupted by API restart").await
}
