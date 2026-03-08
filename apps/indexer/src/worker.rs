use std::time::Duration;
use tracing::{error, info, trace};
use crate::{job, scheduler, watcher, AppState};

pub async fn run_worker(state: AppState, interval_seconds: u64) {
    let wait = Duration::from_secs(interval_seconds.max(1));
    
    // Cleanup stale jobs from previous runs
    if let Err(err) = job::cleanup_stale_jobs(&state.pool).await {
        error!("[CLEANUP] Failed to cleanup stale jobs: {}", err);
    }

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
            if let Err(err) = scheduler::check_and_schedule_auto_scans(&scheduler_state.pool).await {
                error!("[SCHEDULER] Error: {}", err);
            }
            tokio::time::sleep(scheduler_wait).await;
        }
    });

    loop {
        match job::claim_next_job(&state.pool).await {
            Ok(Some((job_id, library_id))) => {
                info!("[INDEXER] Starting job {} library={:?}", job_id, library_id);
                if let Err(err) = job::process_job(&state, job_id, library_id).await {
                    let err_str = err.to_string();
                    if err_str.contains("cancelled") || err_str.contains("Cancelled") {
                        info!("[INDEXER] Job {} was cancelled by user", job_id);
                        // Status is already 'cancelled' in DB, don't change it
                    } else {
                        error!("[INDEXER] Job {} failed: {}", job_id, err);
                        let _ = job::fail_job(&state.pool, job_id, &err_str).await;
                    }
                } else {
                    info!("[INDEXER] Job {} completed", job_id);
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
