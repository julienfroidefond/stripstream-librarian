use std::time::Duration;
use sqlx::Row;
use tracing::{error, info, trace};
use uuid::Uuid;
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
            if let Err(err) = scheduler::check_and_schedule_metadata_refreshes(&scheduler_state.pool).await {
                error!("[SCHEDULER] Metadata refresh error: {}", err);
            }
            if let Err(err) = scheduler::check_and_schedule_reading_status_push(&scheduler_state.pool).await {
                error!("[SCHEDULER] Reading status push error: {}", err);
            }
            if let Err(err) = scheduler::check_and_schedule_download_detection(&scheduler_state.pool).await {
                error!("[SCHEDULER] Download detection error: {}", err);
            }
            if let Err(err) = scheduler::check_and_schedule_prowlarr_rss(&scheduler_state.pool).await {
                error!("[SCHEDULER] Prowlarr RSS error: {}", err);
            }
            if let Err(err) = scheduler::check_and_schedule_telegram_sync(&scheduler_state.pool).await {
                error!("[SCHEDULER] Telegram sync error: {}", err);
            }
            tokio::time::sleep(scheduler_wait).await;
        }
    });

    struct JobInfo {
        job_type: String,
        library_name: Option<String>,
        book_title: Option<String>,
        thumbnail_path: Option<String>,
    }

    async fn load_job_info(
        pool: &sqlx::PgPool,
        job_id: Uuid,
        library_id: Option<Uuid>,
    ) -> JobInfo {
        let row = sqlx::query("SELECT type, book_id FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

        let (job_type, book_id): (String, Option<Uuid>) = match row {
            Some(r) => (r.get("type"), r.get("book_id")),
            None => ("unknown".to_string(), None),
        };

        let library_name: Option<String> = if let Some(lib_id) = library_id {
            sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
                .bind(lib_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        let (book_title, thumbnail_path): (Option<String>, Option<String>) = if let Some(bid) = book_id {
            let row = sqlx::query("SELECT title, thumbnail_path FROM books WHERE id = $1")
                .bind(bid)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
            match row {
                Some(r) => (r.get("title"), r.get("thumbnail_path")),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        JobInfo { job_type, library_name, book_title, thumbnail_path }
    }

    async fn load_scan_stats(pool: &sqlx::PgPool, job_id: Uuid) -> notifications::ScanStats {
        let row = sqlx::query("SELECT stats_json FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

        if let Some(row) = row {
            if let Ok(val) = row.try_get::<serde_json::Value, _>("stats_json") {
                return notifications::ScanStats {
                    scanned_files: val.get("scanned_files").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    indexed_files: val.get("indexed_files").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    removed_files: val.get("removed_files").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    new_series: val.get("new_series").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    errors: val.get("errors").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                };
            }
        }

        notifications::ScanStats {
            scanned_files: 0,
            indexed_files: 0,
            removed_files: 0,
            new_series: 0,
            errors: 0,
        }
    }

    fn build_completed_event(
        job_type: &str,
        library_name: Option<String>,
        book_title: Option<String>,
        thumbnail_path: Option<String>,
        stats: notifications::ScanStats,
        duration_seconds: u64,
    ) -> notifications::NotificationEvent {
        match notifications::job_type_category(job_type) {
            "thumbnail" => notifications::NotificationEvent::ThumbnailCompleted {
                job_type: job_type.to_string(),
                library_name,
                duration_seconds,
            },
            "conversion" => notifications::NotificationEvent::ConversionCompleted {
                library_name,
                book_title,
                thumbnail_path,
            },
            _ => notifications::NotificationEvent::ScanCompleted {
                job_type: job_type.to_string(),
                library_name,
                stats,
                duration_seconds,
            },
        }
    }

    fn build_failed_event(
        job_type: &str,
        library_name: Option<String>,
        book_title: Option<String>,
        thumbnail_path: Option<String>,
        error: String,
    ) -> notifications::NotificationEvent {
        match notifications::job_type_category(job_type) {
            "thumbnail" => notifications::NotificationEvent::ThumbnailFailed {
                job_type: job_type.to_string(),
                library_name,
                error,
            },
            "conversion" => notifications::NotificationEvent::ConversionFailed {
                library_name,
                book_title,
                thumbnail_path,
                error,
            },
            _ => notifications::NotificationEvent::ScanFailed {
                job_type: job_type.to_string(),
                library_name,
                error,
            },
        }
    }

    loop {
        match job::claim_next_job(&state.pool).await {
            Ok(Some((job_id, library_id))) => {
                info!("[INDEXER] Starting job {} library={:?}", job_id, library_id);
                let started_at = std::time::Instant::now();
                let info = load_job_info(&state.pool, job_id, library_id).await;

                if let Err(err) = job::process_job(&state, job_id, library_id).await {
                    let err_str = err.to_string();
                    if err_str.contains("cancelled") || err_str.contains("Cancelled") {
                        info!("[INDEXER] Job {} was cancelled by user", job_id);
                        notifications::notify(
                            state.pool.clone(),
                            notifications::NotificationEvent::ScanCancelled {
                                job_type: info.job_type.clone(),
                                library_name: info.library_name.clone(),
                            },
                        );
                    } else {
                        error!("[INDEXER] Job {} failed: {}", job_id, err);
                        let _ = job::fail_job(&state.pool, job_id, &err_str).await;
                        notifications::notify(
                            state.pool.clone(),
                            build_failed_event(&info.job_type, info.library_name.clone(), info.book_title.clone(), info.thumbnail_path.clone(), err_str),
                        );
                    }
                } else {
                    info!("[INDEXER] Job {} completed", job_id);
                    let stats = load_scan_stats(&state.pool, job_id).await;
                    notifications::notify(
                        state.pool.clone(),
                        build_completed_event(
                            &info.job_type,
                            info.library_name.clone(),
                            info.book_title.clone(),
                            info.thumbnail_path.clone(),
                            stats,
                            started_at.elapsed().as_secs(),
                        ),
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
