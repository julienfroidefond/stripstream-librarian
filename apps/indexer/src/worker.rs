use sqlx::Row;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, trace};
use uuid::Uuid;

use stripstream_core::availability::{self, PresentVolumes, PruneOutcome};
use stripstream_core::coverage::SeriesCoverage;
use stripstream_jobs::{claim_next, fail_job, JobCategory, JobOwner, JobStatus};

use crate::job::{self, RunError};
use crate::{lease, scheduler, watcher, AppState};

pub async fn run_worker(state: AppState, interval_seconds: u64) {
    let wait = Duration::from_secs(interval_seconds.max(1));

    // Cleanup stale jobs from previous runs
    if let Err(err) = job::cleanup_stale_jobs(&state.pool).await {
        error!("[CLEANUP] Failed to cleanup stale jobs: {}", err);
    }

    // Periodic sweep of jobs whose process died (not only at startup): a job
    // interrupted on another instance, or right after it started, is reaped too.
    let sweep_pool = state.pool.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(lease::SWEEP_INTERVAL).await;
            if let Err(err) = job::sweep_orphaned_jobs(&sweep_pool).await {
                error!("[CLEANUP] Failed to sweep orphaned jobs: {}", err);
            }
        }
    });

    // Start file watcher task
    let watcher_state = state.clone();
    let _watcher_handle = tokio::spawn(async move {
        info!("[WATCHER] Starting file watcher service");
        if let Err(err) = watcher::run_file_watcher(watcher_state).await {
            error!("[WATCHER] Error: {}", err);
        }
    });

    // Start scheduler task for auto-monitoring
    let scheduler_state = state.clone();
    let _scheduler_handle = tokio::spawn(async move {
        let scheduler_wait = Duration::from_secs(60); // Check every minute
        loop {
            if let Err(err) = scheduler::check_and_schedule_auto_scans(&scheduler_state.pool).await
            {
                error!("[SCHEDULER] Error: {}", err);
            }
            if let Err(err) =
                scheduler::check_and_schedule_metadata_refreshes(&scheduler_state.pool).await
            {
                error!("[SCHEDULER] Metadata refresh error: {}", err);
            }
            if let Err(err) =
                scheduler::check_and_schedule_reading_status_push(&scheduler_state.pool).await
            {
                error!("[SCHEDULER] Reading status push error: {}", err);
            }
            if let Err(err) =
                scheduler::check_and_schedule_download_detection(&scheduler_state.pool).await
            {
                error!("[SCHEDULER] Download detection error: {}", err);
            }
            if let Err(err) =
                scheduler::check_and_schedule_prowlarr_rss(&scheduler_state.pool).await
            {
                error!("[SCHEDULER] Prowlarr RSS error: {}", err);
            }
            if let Err(err) =
                scheduler::check_and_schedule_telegram_sync_incremental(&scheduler_state.pool).await
            {
                error!("[SCHEDULER] Telegram sync incremental error: {}", err);
            }
            if let Err(err) = scheduler::check_and_schedule_ddl_search(&scheduler_state.pool).await
            {
                error!("[SCHEDULER] DDL search error: {}", err);
            }
            tokio::time::sleep(scheduler_wait).await;
        }
    });

    loop {
        match claim_next(&state.pool, JobOwner::Indexer).await {
            Ok(Some(claimed)) => {
                let (job_id, library_id) = (claimed.id, claimed.library_id);
                info!("[INDEXER] Starting job {} library={:?}", job_id, library_id);
                // Held for the whole run: tells the orphan sweep (of any instance) that
                // this job is alive. Released when the run ends (or on drop).
                let job_lease = match lease::JobLease::acquire(&state.pool, job_id).await {
                    Ok(lease) => lease,
                    Err(err) => {
                        let message = format!("Failed to register the job lease: {err}");
                        error!("[INDEXER] Job {}: {}", job_id, message);
                        let _ = fail_job(&state.pool, job_id, &message, None).await;
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                };
                let started_at = std::time::Instant::now();

                let failure = match job::process_job(&state, job_id, library_id).await {
                    Err(RunError::Cancelled) => {
                        info!("[INDEXER] Job {} was cancelled by user", job_id);
                        notify_job(
                            &state,
                            job_id,
                            library_id,
                            notifications::JobOutcome::Cancelled,
                            started_at,
                        );
                        continue;
                    }
                    Err(RunError::Failed(err)) => {
                        let message = err.to_string();
                        error!("[INDEXER] Job {} failed: {:#}", job_id, err);
                        let _ = fail_job(&state.pool, job_id, &message, None).await;
                        Some(message)
                    }
                    Ok(outcome) => match job::finish(&state.pool, job_id, &outcome).await {
                        Ok(JobStatus::Failed) => {
                            error!("[INDEXER] Job {} failed: every unit failed", job_id);
                            outcome.error_summary()
                        }
                        Ok(status) => {
                            info!("[INDEXER] Job {} finished: {}", job_id, status.as_str());
                            None
                        }
                        Err(err) => {
                            let message = format!("Failed to record the job outcome: {err}");
                            error!("[INDEXER] Job {}: {}", job_id, message);
                            let _ = fail_job(&state.pool, job_id, &message, None).await;
                            Some(message)
                        }
                    },
                };
                // The terminal status is written: the job no longer needs its lease.
                job_lease.release().await;

                if let Some(error) = failure {
                    notify_job(
                        &state,
                        job_id,
                        library_id,
                        notifications::JobOutcome::Failed { error },
                        started_at,
                    );
                } else {
                    // A scan makes the volumes it just imported disappear from the
                    // download offers; the availability module owns the pruning.
                    if notifications::job_type_category(&claimed.job_type) == JobCategory::Scan {
                        if let Some(library_id) = library_id {
                            cleanup_available_downloads(&state.pool, library_id).await;
                        }
                    }

                    notify_job(
                        &state,
                        job_id,
                        library_id,
                        notifications::JobOutcome::Completed,
                        started_at,
                    );
                }
            }
            Ok(None) => {
                trace!("[INDEXER] No pending jobs, waiting...");
                tokio::time::sleep(wait).await;
            }
            Err(err) => {
                error!("[INDEXER] Worker error: {}", err);
                tokio::time::sleep(wait).await;
            }
        }
    }
}

/// Hand a terminal job to the notification module, which owns the policy of
/// which messages a job produces and loads everything it needs.
fn notify_job(
    state: &AppState,
    job_id: Uuid,
    library_id: Option<Uuid>,
    outcome: notifications::JobOutcome,
    started_at: std::time::Instant,
) {
    notifications::notify_job(
        state.pool.clone(),
        notifications::JobFinished {
            job_id,
            library_id,
            outcome,
            duration_seconds: started_at.elapsed().as_secs(),
        },
    );
}

/// After a scan completes, prune `available_downloads` entries by removing volumes
/// that are now present in the library. The availability module owns the release
/// JSON walk, the counter derivation and the empty-row deletion.
async fn cleanup_available_downloads(pool: &sqlx::PgPool, library_id: Uuid) {
    let series_ids: Vec<Uuid> =
        match sqlx::query_scalar("SELECT series_id FROM available_downloads WHERE library_id = $1")
            .bind(library_id)
            .fetch_all(pool)
            .await
        {
            Ok(ids) => ids,
            Err(e) => {
                error!("[CLEANUP] Failed to fetch available_downloads: {e}");
                return;
            }
        };

    if series_ids.is_empty() {
        return;
    }

    // Preload coverage facts for every series referenced by these rows in one
    // query. `SeriesCoverage` owns the volume-type and integral rules.
    let coverage_by_series: HashMap<Uuid, SeriesCoverage> = match sqlx::query(
        "SELECT series_id, volume_type, volume FROM books WHERE series_id = ANY($1)",
    )
    .bind(&series_ids)
    .fetch_all(pool)
    .await
    {
        Ok(book_rows) => {
            let mut map: HashMap<Uuid, SeriesCoverage> = HashMap::new();
            for book_row in book_rows {
                let sid: Uuid = book_row.get("series_id");
                let volume_type: String = book_row.get("volume_type");
                let volume: Option<i32> = book_row.get("volume");
                map.entry(sid)
                    .or_insert_with(|| SeriesCoverage::new(None))
                    .add(&volume_type, volume);
            }
            map
        }
        Err(e) => {
            error!("[CLEANUP] Failed to fetch series coverage: {e}");
            return;
        }
    };

    let present_by_series: HashMap<Uuid, PresentVolumes> = coverage_by_series
        .iter()
        .map(|(series_id, coverage)| (*series_id, PresentVolumes::from(coverage)))
        .collect();

    match availability::prune_library(pool, library_id, &present_by_series).await {
        Ok(outcomes) => {
            for series in outcomes {
                match series.outcome {
                    PruneOutcome::Deleted => info!(
                        "[CLEANUP] Deleted available_downloads for series {} (all volumes present)",
                        series.series_id
                    ),
                    PruneOutcome::Updated {
                        removed_volumes,
                        missing_count,
                    } => info!(
                        "[CLEANUP] Pruned {} volume(s) from available_downloads for series {} (missing_count={missing_count})",
                        removed_volumes.len(),
                        series.series_id
                    ),
                    PruneOutcome::Missing | PruneOutcome::Unchanged => {}
                }
            }
        }
        Err(e) => error!("[CLEANUP] Failed to prune available_downloads: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
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

    async fn seed_series(pool: &sqlx::PgPool, library_id: Uuid, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(library_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn seed_book(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        series_id: Uuid,
        volume_type: &str,
        volume: Option<i32>,
    ) {
        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, title, kind, format, volume, volume_type) \
             VALUES ($1, $2, $3, $4, 'comic', 'cbz', $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .bind(series_id)
        .bind(format!("{volume_type}-{volume:?}"))
        .bind(volume)
        .bind(volume_type)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn seed_download(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        series_id: Uuid,
        missing_count: i32,
    ) {
        sqlx::query(
            "INSERT INTO available_downloads \
             (library_id, series_id, missing_count, available_releases) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(library_id)
        .bind(series_id)
        .bind(missing_count)
        .bind(serde_json::json!([
            {"title": "T01", "matched_missing_volumes": [1, 2]}
        ]))
        .execute(pool)
        .await
        .unwrap();
    }

    async fn stored_releases(
        pool: &sqlx::PgPool,
        series_id: Uuid,
    ) -> Option<(i32, serde_json::Value)> {
        sqlx::query_as(
            "SELECT missing_count, available_releases FROM available_downloads WHERE series_id = $1",
        )
        .bind(series_id)
        .fetch_optional(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn cleanup_prunes_a_numbered_integral_only_when_its_volume_is_present(
        pool: sqlx::PgPool,
    ) {
        let library = seed_library(&pool, "lib").await;
        let series = seed_series(&pool, library, "Series").await;
        seed_book(&pool, library, series, "integral", Some(2)).await;
        seed_download(&pool, library, series, 2).await;

        cleanup_available_downloads(&pool, library).await;

        let (missing_count, releases) = stored_releases(&pool, series).await.unwrap();
        assert_eq!(missing_count, 1);
        assert_eq!(
            releases[0]["matched_missing_volumes"],
            serde_json::json!([1]),
            "volume 2 is covered by the numbered integral"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn cleanup_does_not_count_oneshot_volumes_as_present(pool: sqlx::PgPool) {
        let library = seed_library(&pool, "lib").await;
        let series = seed_series(&pool, library, "Series").await;
        seed_book(&pool, library, series, "oneshot", Some(2)).await;
        seed_download(&pool, library, series, 2).await;

        cleanup_available_downloads(&pool, library).await;

        let (missing_count, releases) = stored_releases(&pool, series).await.unwrap();
        assert_eq!(missing_count, 2, "one-shots never cover a tome");
        assert_eq!(
            releases[0]["matched_missing_volumes"],
            serde_json::json!([1, 2])
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn cleanup_counts_a_shared_volume_once(pool: sqlx::PgPool) {
        let library = seed_library(&pool, "lib").await;
        let series = seed_series(&pool, library, "Series").await;
        seed_book(&pool, library, series, "regular", Some(1)).await;
        sqlx::query(
            "INSERT INTO available_downloads \
             (library_id, series_id, missing_count, available_releases) \
             VALUES ($1, $2, 3, $3)",
        )
        .bind(library)
        .bind(series)
        .bind(serde_json::json!([
            {"title": "R1", "matched_missing_volumes": [1, 2]},
            {"title": "R2", "matched_missing_volumes": [1, 3]}
        ]))
        .execute(&pool)
        .await
        .unwrap();

        cleanup_available_downloads(&pool, library).await;

        let (missing_count, releases) = stored_releases(&pool, series).await.unwrap();
        assert_eq!(
            missing_count, 2,
            "the same volume offered by two releases decrements once"
        );
        assert_eq!(
            releases,
            serde_json::json!([
                {"title": "R1", "matched_missing_volumes": [2]},
                {"title": "R2", "matched_missing_volumes": [3]}
            ])
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn cleanup_deletes_the_download_when_an_unnumbered_integral_is_present(
        pool: sqlx::PgPool,
    ) {
        let library = seed_library(&pool, "lib").await;
        let series = seed_series(&pool, library, "Series").await;
        seed_book(&pool, library, series, "integral", None).await;
        seed_download(&pool, library, series, 2).await;

        cleanup_available_downloads(&pool, library).await;

        assert!(
            stored_releases(&pool, series).await.is_none(),
            "an unnumbered integral completes the whole series"
        );
    }
}
