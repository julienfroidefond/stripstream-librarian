use std::time::Duration;

use sqlx::PgPool;
use tracing::{error, info, trace};

use stripstream_jobs::{claim_next, fail_job, fail_orphaned_jobs, JobOwner};

use crate::{
    downloads::{ddl_provider, detection as download_detection, rss_poll, telegram_monitor},
    jobs::launcher::{self, JobSpec},
    metadata, reading,
};

/// `ddl_search` is only ever polled; it reports failures like the other download searches.
const DDL_SEARCH: JobSpec = JobSpec {
    job_type: "ddl_search",
    conflicts: &[],
    failure: Some(download_detection::detection_failed),
    partial_stats: None,
};

/// The launcher spec whose failure path (partial stats, `fail_job`, notification)
/// applies to a polled job. `None` means a plain `fail_job`.
fn failure_spec(job_type: &str) -> Option<&'static JobSpec> {
    match job_type {
        "metadata_refresh" => Some(&metadata::refresh::METADATA_REFRESH),
        "metadata_refresh_all" => Some(&metadata::refresh::METADATA_REFRESH_ALL),
        "metadata_batch" => Some(&metadata::batch::METADATA_BATCH),
        "metadata_batch_rematch" => Some(&metadata::batch::METADATA_BATCH_REMATCH),
        "reading_status_push" => Some(&reading::status_push::READING_STATUS_PUSH),
        "download_detection" => Some(&download_detection::DOWNLOAD_DETECTION),
        "prowlarr_rss" => Some(&rss_poll::PROWLARR_RSS),
        "ddl_search" => Some(&DDL_SEARCH),
        _ => None,
    }
}

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
                        match failure_spec(&job_type) {
                            Some(spec) => {
                                launcher::report_failure(&pool_clone, job_id, library_id, spec, &e)
                                    .await
                            }
                            None => {
                                let _ = fail_job(&pool_clone, job_id, &e, None).await;
                            }
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
