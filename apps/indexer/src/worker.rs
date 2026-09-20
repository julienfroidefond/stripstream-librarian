use crate::{job, scheduler, watcher, AppState};
use sqlx::Row;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, trace};
use uuid::Uuid;

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
            tokio::time::sleep(scheduler_wait).await;
        }
    });

    struct JobInfo {
        job_type: String,
        library_name: Option<String>,
        book_title: Option<String>,
        thumbnail_path: Option<String>,
    }

    struct ScanSeriesNotification {
        series_name: String,
        thumbnail_path: Option<String>,
        book_titles: Vec<String>,
    }

    async fn load_job_info(pool: &sqlx::PgPool, job_id: Uuid, library_id: Option<Uuid>) -> JobInfo {
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

        let (book_title, thumbnail_path): (Option<String>, Option<String>) =
            if let Some(bid) = book_id {
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

        JobInfo {
            job_type,
            library_name,
            book_title,
            thumbnail_path,
        }
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
                    scanned_files: val
                        .get("scanned_files")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize,
                    indexed_files: val
                        .get("indexed_files")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize,
                    removed_files: val
                        .get("removed_files")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize,
                    new_series: val.get("new_series").and_then(|v| v.as_u64()).unwrap_or(0)
                        as usize,
                    errors: val.get("errors").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                    new_series_names: val
                        .get("new_series_names")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default(),
                    new_book_titles: val
                        .get("new_book_titles")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default(),
                };
            }
        }

        notifications::ScanStats {
            scanned_files: 0,
            indexed_files: 0,
            removed_files: 0,
            new_series: 0,
            errors: 0,
            new_series_names: Vec::new(),
            new_book_titles: Vec::new(),
        }
    }

    async fn load_scan_series_notifications(
        pool: &sqlx::PgPool,
        job_id: Uuid,
    ) -> Vec<ScanSeriesNotification> {
        let rows = sqlx::query(
            "SELECT \
                COALESCE(s.name, 'Non classé') AS series_name, \
                b.title, \
                b.thumbnail_path \
             FROM index_job_events e \
             JOIN books b ON b.id = e.entity_id \
             LEFT JOIN series s ON s.id = b.series_id \
             WHERE e.job_id = $1 AND e.event_type = 'book_added' \
             ORDER BY COALESCE(s.name, 'Non classé'), b.volume NULLS LAST, b.title",
        )
        .bind(job_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut groups: HashMap<String, ScanSeriesNotification> = HashMap::new();
        for row in rows {
            let series_name: String = row.get("series_name");
            let title: String = row.get("title");
            let thumbnail_path: Option<String> = row.get("thumbnail_path");

            let group =
                groups
                    .entry(series_name.clone())
                    .or_insert_with(|| ScanSeriesNotification {
                        series_name,
                        thumbnail_path: None,
                        book_titles: Vec::new(),
                    });

            if group.thumbnail_path.is_none() && thumbnail_path.is_some() {
                group.thumbnail_path = thumbnail_path;
            }
            group.book_titles.push(title);
        }

        let mut grouped: Vec<_> = groups.into_values().collect();
        grouped.sort_by(|a, b| a.series_name.cmp(&b.series_name));
        grouped
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
                            build_failed_event(
                                &info.job_type,
                                info.library_name.clone(),
                                info.book_title.clone(),
                                info.thumbnail_path.clone(),
                                err_str,
                            ),
                        );
                    }
                } else {
                    info!("[INDEXER] Job {} completed", job_id);
                    let stats = load_scan_stats(&state.pool, job_id).await;
                    if notifications::job_type_category(&info.job_type) == "scan" {
                        if let Some(lid) = library_id {
                            cleanup_available_downloads(&state.pool, lid).await;
                        }
                        let series_notifications =
                            load_scan_series_notifications(&state.pool, job_id).await;

                        for series in &series_notifications {
                            notifications::notify(
                                state.pool.clone(),
                                notifications::NotificationEvent::ScanSeriesDiscovered {
                                    library_name: info.library_name.clone(),
                                    series_name: series.series_name.clone(),
                                    thumbnail_path: series.thumbnail_path.clone(),
                                    book_titles: series.book_titles.clone(),
                                },
                            );
                        }

                        if series_notifications.is_empty()
                            || stats.removed_files > 0
                            || stats.errors > 0
                        {
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
                    } else {
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

/// After a scan completes, prune `available_downloads` entries by removing volumes
/// that are now present in the library. Deletes releases with no remaining matched
/// volumes, and deletes rows with no remaining releases.
async fn cleanup_available_downloads(pool: &sqlx::PgPool, library_id: Uuid) {
    let rows = match sqlx::query(
        "SELECT id, series_id, available_releases, missing_count \
         FROM available_downloads WHERE library_id = $1",
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("[CLEANUP] Failed to fetch available_downloads: {e}");
            return;
        }
    };

    // Preload present volumes for every series referenced by these rows in one query.
    let series_ids: Vec<Uuid> = rows.iter().map(|r| r.get("series_id")).collect();
    let present_by_series: std::collections::HashMap<Uuid, std::collections::HashSet<i32>> =
        match sqlx::query(
            "SELECT series_id, volume FROM books \
             WHERE series_id = ANY($1) AND volume IS NOT NULL \
             AND volume_type IN ('regular', 'integral', 'oneshot')",
        )
        .bind(&series_ids)
        .fetch_all(pool)
        .await
        {
            Ok(vol_rows) => {
                let mut map: std::collections::HashMap<Uuid, std::collections::HashSet<i32>> =
                    std::collections::HashMap::new();
                for vr in vol_rows {
                    let sid: Uuid = vr.get("series_id");
                    let vol: i32 = vr.get("volume");
                    map.entry(sid).or_default().insert(vol);
                }
                map
            }
            Err(e) => {
                error!("[CLEANUP] Failed to fetch present volumes: {e}");
                return;
            }
        };

    for row in rows {
        let ad_id: Uuid = row.get("id");
        let series_id: Uuid = row.get("series_id");
        let releases_json: Option<serde_json::Value> = row.get("available_releases");
        let old_missing: i32 = row.get("missing_count");

        let Some(present_volumes) = present_by_series.get(&series_id) else {
            continue;
        };

        if present_volumes.is_empty() {
            continue;
        }

        let Some(serde_json::Value::Array(releases)) = releases_json else {
            continue;
        };

        let mut pruned_vols: std::collections::HashSet<i32> = std::collections::HashSet::new();
        let updated: Vec<serde_json::Value> = releases
            .into_iter()
            .filter_map(|mut release| {
                if let Some(matched) = release.get_mut("matched_missing_volumes") {
                    if let Some(arr) = matched.as_array() {
                        let filtered: Vec<serde_json::Value> = arr
                            .iter()
                            .filter(|v| {
                                let vol = v.as_i64().unwrap_or(-1) as i32;
                                let present = present_volumes.contains(&vol);
                                if present {
                                    pruned_vols.insert(vol);
                                }
                                !present
                            })
                            .cloned()
                            .collect();
                        if filtered.is_empty() {
                            return None;
                        }
                        *matched = serde_json::Value::Array(filtered);
                    }
                }
                Some(release)
            })
            .collect();

        if pruned_vols.is_empty() {
            continue;
        }

        if updated.is_empty() {
            let _ = sqlx::query("DELETE FROM available_downloads WHERE id = $1")
                .bind(ad_id)
                .execute(pool)
                .await;
            info!("[CLEANUP] Deleted available_downloads {ad_id} (all volumes now present)");
        } else {
            let new_missing = (old_missing - pruned_vols.len() as i32).max(0);
            let _ = sqlx::query(
                "UPDATE available_downloads \
                 SET available_releases = $1, missing_count = $2, updated_at = NOW() \
                 WHERE id = $3",
            )
            .bind(serde_json::Value::Array(updated))
            .bind(new_missing)
            .bind(ad_id)
            .execute(pool)
            .await;
            info!(
                "[CLEANUP] Pruned {} volume(s) from available_downloads {ad_id}",
                pruned_vols.len()
            );
        }
    }
}
