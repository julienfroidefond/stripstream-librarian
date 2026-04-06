use axum::{extract::{Path, State}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{info, trace, warn};
use uuid::Uuid;

use crate::{error::ApiError, metadata, state::AppState};
use super::qbittorrent::{load_qbittorrent_config, qbittorrent_login, resolve_hash_by_category};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Called by qBittorrent on torrent completion.
/// Configure in qBittorrent: Tools → Options → Downloads → "Run external program on torrent completion":
///   curl -s -X POST http://api:7080/torrent-downloads/notify \
///        -H "Content-Type: application/json" \
///        -d "{\"hash\":\"%I\",\"name\":\"%N\",\"save_path\":\"%F\"}"
#[derive(Deserialize)]
pub struct TorrentNotifyRequest {
    pub hash: String,
    #[allow(dead_code)]
    pub name: String,
    /// %F from qBittorrent: path to content (folder for multi-file, file for single-file)
    pub save_path: String,
}

#[derive(Serialize)]
pub struct TorrentDownloadDto {
    pub id: String,
    pub library_id: String,
    pub series_id: Option<String>,
    pub series_name: String,
    pub expected_volumes: Vec<i32>,
    pub qb_hash: Option<String>,
    pub content_path: Option<String>,
    pub status: String,
    pub imported_files: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub progress: f32,
    pub download_speed: i64,
    pub eta: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct ImportedFile {
    pub(super) volume: i32,
    pub(super) source: String,
    pub(super) destination: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// Webhook called by qBittorrent when a torrent completes (no auth required).
pub async fn notify_torrent_done(
    State(state): State<AppState>,
    Json(body): Json<TorrentNotifyRequest>,
) -> Result<Json<crate::responses::OkResponse>, ApiError> {
    if body.hash.is_empty() {
        return Err(ApiError::bad_request("hash is required"));
    }

    if !is_torrent_import_enabled(&state.pool).await {
        info!("Torrent import disabled, ignoring notification for hash {}", body.hash);
        return Ok(Json(crate::responses::OkResponse::new()));
    }

    let row = sqlx::query(
        "SELECT id FROM torrent_downloads WHERE qb_hash = $1 AND status = 'downloading' LIMIT 1",
    )
    .bind(&body.hash)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        info!("Torrent notification for unknown hash {}, ignoring", body.hash);
        return Ok(Json(crate::responses::OkResponse::new()));
    };

    let torrent_id: Uuid = row.get("id");

    sqlx::query(
        "UPDATE torrent_downloads SET status = 'completed', content_path = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&body.save_path)
    .bind(torrent_id)
    .execute(&state.pool)
    .await?;

    info!("Torrent {} completed, content at {}", body.hash, body.save_path);

    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Err(e) = process_torrent_import(pool, torrent_id).await {
            warn!("Torrent import {} failed: {:#}", torrent_id, e);
        }
    });

    Ok(Json(crate::responses::OkResponse::new()))
}

/// List recent torrent downloads (admin).
pub async fn list_torrent_downloads(
    State(state): State<AppState>,
) -> Result<Json<Vec<TorrentDownloadDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT td.id, td.library_id, td.series_name, td.expected_volumes, td.qb_hash, td.content_path, \
         td.status, td.imported_files, td.error_message, td.progress, td.download_speed, td.eta, \
         td.created_at, td.updated_at, s.id AS series_id \
         FROM torrent_downloads td \
         LEFT JOIN series s ON s.library_id = td.library_id AND LOWER(s.name) = LOWER(td.series_name) \
         ORDER BY td.created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;

    let dtos = rows
        .into_iter()
        .map(|row| {
            let id: Uuid = row.get("id");
            let library_id: Uuid = row.get("library_id");
            let series_id: Option<Uuid> = row.get("series_id");
            let expected_volumes: Vec<i32> = row.get("expected_volumes");
            let created_at: DateTime<Utc> = row.get("created_at");
            let updated_at: DateTime<Utc> = row.get("updated_at");
            TorrentDownloadDto {
                id: id.to_string(),
                library_id: library_id.to_string(),
                series_id: series_id.map(|u| u.to_string()),
                series_name: row.get("series_name"),
                expected_volumes,
                qb_hash: row.get("qb_hash"),
                content_path: row.get("content_path"),
                status: row.get("status"),
                imported_files: row.get("imported_files"),
                error_message: row.get("error_message"),
                progress: row.get("progress"),
                download_speed: row.get("download_speed"),
                eta: row.get("eta"),
                created_at: created_at.to_rfc3339(),
                updated_at: updated_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(dtos))
}

/// Delete a torrent download entry. If the torrent is still downloading, also remove it from qBittorrent.
pub async fn delete_torrent_download(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::responses::OkResponse>, ApiError> {
    let row = sqlx::query("SELECT qb_hash, status FROM torrent_downloads WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;

    let Some(row) = row else {
        return Err(ApiError::not_found("torrent download not found"));
    };

    let qb_hash: Option<String> = row.get("qb_hash");
    let status: String = row.get("status");

    // If downloading, try to cancel in qBittorrent
    if status == "downloading" {
        if let Some(ref hash) = qb_hash {
            if let Ok((base_url, username, password)) = load_qbittorrent_config(&state.pool).await {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                    .ok();
                if let Some(client) = client {
                    if let Ok(sid) = qbittorrent_login(&client, &base_url, &username, &password).await {
                        let _ = client
                            .post(format!("{base_url}/api/v2/torrents/delete"))
                            .header("Cookie", format!("SID={sid}"))
                            .form(&[("hashes", hash.as_str()), ("deleteFiles", "true")])
                            .send()
                            .await;
                        info!("Deleted torrent {} from qBittorrent", hash);
                    }
                }
            }
        }
    }

    sqlx::query("DELETE FROM torrent_downloads WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    info!("Deleted torrent download {id}");
    Ok(Json(crate::responses::OkResponse::new()))
}

// ─── Background poller ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct QbTorrentInfo {
    hash: String,
    state: String,
    content_path: Option<String>,
    save_path: Option<String>,
    name: Option<String>,
    #[serde(default)]
    progress: f64,
    #[serde(default)]
    #[allow(dead_code)]
    total_size: i64,
    #[serde(default)]
    dlspeed: i64,
    #[serde(default)]
    eta: i64,
}

/// Completed states in qBittorrent: torrent is fully downloaded and seeding.
const QB_COMPLETED_STATES: &[&str] = &[
    "uploading", "stalledUP", "pausedUP", "queuedUP", "checkingUP", "forcedUP",
];

/// Failed/stalled states: torrent cannot make progress (no seeds, stalled download).
const QB_FAILED_STATES: &[&str] = &[
    "stalledDL", "pausedDL", "error", "missingFiles",
];

pub async fn run_torrent_poller(pool: PgPool, interval_seconds: u64) {
    let idle_wait = Duration::from_secs(interval_seconds.max(5));
    let active_wait = Duration::from_secs(3);
    let error_wait = Duration::from_secs(10);
    loop {
        let wait = match poll_qbittorrent_downloads(&pool).await {
            Ok(true) => active_wait,
            Ok(false) => idle_wait,
            Err(e) => {
                warn!("[TORRENT_POLLER] {:#}", e);
                // Check if there are active downloads — if so, retry faster
                let has_rows = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM torrent_downloads WHERE status = 'downloading'"
                )
                .fetch_one(&pool)
                .await
                .unwrap_or(0);
                if has_rows > 0 { error_wait } else { idle_wait }
            }
        };
        tokio::time::sleep(wait).await;
    }
}

/// Returns Ok(true) if there are active downloads, Ok(false) otherwise.
async fn poll_qbittorrent_downloads(pool: &PgPool) -> anyhow::Result<bool> {
    if !is_torrent_import_enabled(pool).await {
        return Ok(false);
    }

    let rows = sqlx::query(
        "SELECT id, qb_hash FROM torrent_downloads WHERE status = 'downloading'",
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        trace!("[TORRENT_POLLER] No active downloads to poll");
        return Ok(false);
    }

    let (base_url, username, password) = load_qbittorrent_config(pool)
        .await
        .map_err(|e| anyhow::anyhow!("qBittorrent config: {}", e.message))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let sid = qbittorrent_login(&client, &base_url, &username, &password)
        .await
        .map_err(|e| anyhow::anyhow!("qBittorrent login: {}", e.message))?;

    // Try to resolve hash for rows that are missing it (category-based retry)
    for row in &rows {
        let qb_hash: Option<String> = row.get("qb_hash");
        if qb_hash.is_some() {
            continue;
        }
        let tid: Uuid = row.get("id");
        let category = format!("sl-{tid}");
        if let Some(hash) = resolve_hash_by_category(&client, &base_url, &sid, &category).await {
            info!("[TORRENT_POLLER] Late-resolved hash {hash} for torrent {tid} via category {category}");
            let _ = sqlx::query(
                "UPDATE torrent_downloads SET qb_hash = $1, updated_at = NOW() WHERE id = $2",
            )
            .bind(&hash)
            .bind(tid)
            .execute(pool)
            .await;
        }
    }

    // Re-fetch rows to include newly resolved hashes
    let rows = sqlx::query(
        "SELECT id, qb_hash FROM torrent_downloads WHERE status = 'downloading'",
    )
    .fetch_all(pool)
    .await?;

    // Filter to rows that have a resolved hash
    let rows: Vec<_> = rows.into_iter().filter(|r| {
        let qb_hash: Option<String> = r.get("qb_hash");
        qb_hash.is_some()
    }).collect();

    if rows.is_empty() {
        return Ok(true);
    }

    let hashes: Vec<String> = rows
        .iter()
        .map(|r| { let h: String = r.get("qb_hash"); h })
        .collect();
    let hashes_param = hashes.join("|");

    let resp = client
        .get(format!("{base_url}/api/v2/torrents/info"))
        .query(&[("hashes", &hashes_param)])
        .header("Cookie", format!("SID={sid}"))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("qBittorrent API returned {}", resp.status()));
    }

    let infos: Vec<QbTorrentInfo> = resp.json().await?;

    for info in &infos {
        info!("[TORRENT_POLLER] Torrent {} state='{}' progress={:.2} name={:?}", info.hash, info.state, info.progress, info.name);

        // Update progress for all active torrents
        let row = rows.iter().find(|r| {
            let h: String = r.get("qb_hash");
            h == info.hash
        });
        if let Some(row) = row {
            let tid: Uuid = row.get("id");
            let global_progress = info.progress as f32;
            let _ = sqlx::query(
                "UPDATE torrent_downloads SET progress = $1, download_speed = $2, eta = $3, updated_at = NOW() \
                 WHERE id = $4 AND status = 'downloading'",
            )
            .bind(global_progress)
            .bind(info.dlspeed)
            .bind(info.eta)
            .bind(tid)
            .execute(pool)
            .await;
        }

        if QB_FAILED_STATES.contains(&info.state.as_str()) {
            let Some(row) = rows.iter().find(|r| {
                let h: String = r.get("qb_hash");
                h == info.hash
            }) else { continue; };
            let tid: Uuid = row.get("id");
            let msg = format!("Torrent stalled in qBittorrent (state: {})", info.state);
            warn!("[TORRENT_POLLER] Torrent {} failed: {}", info.hash, msg);
            let _ = sqlx::query(
                "UPDATE torrent_downloads SET status = 'error', error_message = $1, \
                 download_speed = 0, eta = 0, updated_at = NOW() \
                 WHERE id = $2 AND status = 'downloading'",
            )
            .bind(&msg)
            .bind(tid)
            .execute(pool)
            .await;

            // Remove torrent from qBittorrent
            let _ = client
                .post(format!("{base_url}/api/v2/torrents/delete"))
                .header("Cookie", format!("SID={sid}"))
                .form(&[("hashes", info.hash.as_str()), ("deleteFiles", "true")])
                .send()
                .await;
            info!("[TORRENT_POLLER] Removed failed torrent {} from qBittorrent", info.hash);
            continue;
        }

        if !QB_COMPLETED_STATES.contains(&info.state.as_str()) {
            continue;
        }

        // content_path is available since qBittorrent 4.3.2; fall back to save_path + name
        let content_path = info.content_path.as_deref()
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                let save = info.save_path.as_deref().unwrap_or("").trim_end_matches('/');
                let name = info.name.as_deref().unwrap_or("");
                if name.is_empty() { None } else { Some(format!("{save}/{name}")) }
            });

        let Some(content_path) = content_path else {
            warn!("[TORRENT_POLLER] Torrent {} completed but content_path unknown", info.hash);
            continue;
        };

        let Some(row) = rows.iter().find(|r| {
            let h: String = r.get("qb_hash");
            h == info.hash
        }) else { continue; };
        let torrent_id: Uuid = row.get("id");

        let updated = sqlx::query(
            "UPDATE torrent_downloads SET status = 'completed', content_path = $1, progress = 1, \
             download_speed = 0, eta = 0, updated_at = NOW() \
             WHERE id = $2 AND status = 'downloading'",
        )
        .bind(&content_path)
        .bind(torrent_id)
        .execute(pool)
        .await?;

        if updated.rows_affected() > 0 {
            info!("[TORRENT_POLLER] Torrent {} completed, content at {}, starting import", info.hash, content_path);
            let pool_clone = pool.clone();
            tokio::spawn(async move {
                if let Err(e) = process_torrent_import(pool_clone, torrent_id).await {
                    warn!("Torrent import {} failed: {:#}", torrent_id, e);
                }
            });
        }
    }

    // Still active if any rows remain in 'downloading' status
    let still_active = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM torrent_downloads WHERE status = 'downloading'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    Ok(still_active > 0)
}

// ─── Import processing ────────────────────────────────────────────────────────

async fn is_torrent_import_enabled(pool: &PgPool) -> bool {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'torrent_import'")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    row.map(|r| {
        let v: serde_json::Value = r.get("value");
        v.get("enabled").and_then(|e| e.as_bool()).unwrap_or(false)
    })
    .unwrap_or(false)
}

async fn process_torrent_import(pool: PgPool, torrent_id: Uuid) -> anyhow::Result<()> {
    let row = sqlx::query(
        "SELECT td.library_id, td.series_name, td.expected_volumes, td.content_path, td.qb_hash, td.replace_existing, l.name AS library_name \
         FROM torrent_downloads td LEFT JOIN libraries l ON l.id = td.library_id WHERE td.id = $1",
    )
    .bind(torrent_id)
    .fetch_one(&pool)
    .await?;

    let library_id: Uuid = row.get("library_id");
    let series_name: String = row.get("series_name");
    let library_name: Option<String> = row.get("library_name");
    let expected_volumes: Vec<i32> = row.get("expected_volumes");
    let content_path: Option<String> = row.get("content_path");
    let qb_hash: Option<String> = row.get("qb_hash");
    let replace_existing: bool = row.get("replace_existing");
    let content_path =
        content_path.ok_or_else(|| anyhow::anyhow!("content_path not set on torrent_download"))?;

    sqlx::query(
        "UPDATE torrent_downloads SET status = 'importing', updated_at = NOW() WHERE id = $1",
    )
    .bind(torrent_id)
    .execute(&pool)
    .await?;

    match do_import(&pool, library_id, &series_name, &expected_volumes, &content_path, replace_existing).await {
        Ok(imported) => {
            let json = serde_json::to_value(&imported).unwrap_or(serde_json::json!([]));
            sqlx::query(
                "UPDATE torrent_downloads SET status = 'imported', imported_files = $1, updated_at = NOW() WHERE id = $2",
            )
            .bind(json)
            .bind(torrent_id)
            .execute(&pool)
            .await?;

            // Queue a scan job so the indexer picks up the new files
            let scan_job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
            )
            .bind(scan_job_id)
            .bind(library_id)
            .execute(&pool)
            .await?;

            // Insert events for each imported file
            for imp in &imported {
                let detail = serde_json::json!({
                    "source_filename": std::path::Path::new(&imp.source)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(""),
                    "volumes": [imp.volume],
                });
                let _ = sqlx::query(
                    "INSERT INTO index_job_events (job_id, event_type, level, entity_type, entity_name, detail) \
                     VALUES ($1, 'file_imported', 'info', 'book', $2, $3)",
                )
                .bind(scan_job_id)
                .bind(&imp.destination)
                .bind(detail)
                .execute(&pool)
                .await;
            }

            // Refresh metadata for this series if it has an approved metadata link
            let link_row = sqlx::query(
                "SELECT eml.id, eml.provider, eml.external_id FROM external_metadata_links eml \
                 JOIN series s ON s.id = eml.series_id \
                 WHERE s.library_id = $1 AND LOWER(s.name) = LOWER($2) AND eml.status = 'approved' LIMIT 1",
            )
            .bind(library_id)
            .bind(&series_name)
            .fetch_optional(&pool)
            .await?;

            if let Some(link) = link_row {
                let link_id: Uuid = link.get("id");
                let provider: String = link.get("provider");
                let external_id: String = link.get("external_id");
                let pool2 = pool.clone();
                let sn = series_name.clone();
                tokio::spawn(async move {
                    let result = metadata::refresh_link(&pool2, link_id, library_id, &sn, &provider, &external_id).await;
                    if let Err(e) = result {
                        warn!("[IMPORT] Metadata refresh for '{}' failed: {}", sn, e);
                    } else {
                        info!("[IMPORT] Metadata refresh for '{}' done", sn);
                    }
                });
            }

            // Update available_downloads: remove imported volumes
            let imported_vols: Vec<i32> = imported.iter().map(|f| f.volume).collect();
            if !imported_vols.is_empty() {
                let ad_row = sqlx::query(
                    "SELECT ad.id, ad.missing_count, ad.available_releases FROM available_downloads ad \
                     JOIN series s ON s.id = ad.series_id \
                     WHERE s.library_id = $1 AND LOWER(s.name) = LOWER($2)",
                )
                .bind(library_id)
                .bind(&series_name)
                .fetch_optional(&pool)
                .await
                .unwrap_or(None);

                if let Some(ad_row) = ad_row {
                    let ad_id: Uuid = ad_row.get("id");
                    let releases_json: Option<serde_json::Value> = ad_row.get("available_releases");
                    if let Some(serde_json::Value::Array(releases)) = releases_json {
                        let updated: Vec<serde_json::Value> = releases.into_iter().filter_map(|mut release| {
                            if let Some(matched) = release.get_mut("matched_missing_volumes") {
                                if let Some(arr) = matched.as_array() {
                                    let filtered: Vec<serde_json::Value> = arr.iter()
                                        .filter(|v| !imported_vols.contains(&(v.as_i64().unwrap_or(-1) as i32)))
                                        .cloned()
                                        .collect();
                                    if filtered.is_empty() {
                                        return None;
                                    }
                                    *matched = serde_json::Value::Array(filtered);
                                }
                            }
                            Some(release)
                        }).collect();

                        if updated.is_empty() {
                            let _ = sqlx::query("DELETE FROM available_downloads WHERE id = $1")
                                .bind(ad_id).execute(&pool).await;
                        } else {
                            let new_missing = ad_row.get::<i32, _>("missing_count") - imported_vols.len() as i32;
                            let _ = sqlx::query(
                                "UPDATE available_downloads SET available_releases = $1, missing_count = GREATEST($2, 0), updated_at = NOW() WHERE id = $3",
                            )
                            .bind(serde_json::Value::Array(updated))
                            .bind(new_missing)
                            .bind(ad_id)
                            .execute(&pool)
                            .await;
                        }
                    }
                }
            }

            // Clean up: remove the sl-{id} category directory and all its contents
            let downloads_root = remap_downloads_path("/downloads");
            let category_dir = remap_downloads_path(&format!("/downloads/sl-{torrent_id}"));
            let category_p = std::path::Path::new(&category_dir);
            let downloads_p = std::path::Path::new(&downloads_root);
            if category_p.is_dir() && category_p != downloads_p && category_p.starts_with(downloads_p) {
                match std::fs::remove_dir_all(category_p) {
                    Ok(()) => info!("[IMPORT] Cleaned up category directory: {}", category_dir),
                    Err(e) => warn!("[IMPORT] Failed to clean up {}: {}", category_dir, e),
                }
            }

            // Remove torrent and category from qBittorrent
            if let Some(ref hash) = qb_hash {
                if let Ok((base_url, username, password)) = load_qbittorrent_config(&pool).await {
                    if let Ok(client) = reqwest::Client::builder().timeout(Duration::from_secs(10)).build() {
                        if let Ok(sid) = qbittorrent_login(&client, &base_url, &username, &password).await {
                            let _ = client
                                .post(format!("{base_url}/api/v2/torrents/delete"))
                                .header("Cookie", format!("SID={sid}"))
                                .form(&[("hashes", hash.as_str()), ("deleteFiles", "true")])
                                .send()
                                .await;
                            info!("[IMPORT] Removed torrent {} from qBittorrent", hash);

                            // Remove the sl-{id} category
                            let cat = format!("sl-{torrent_id}");
                            let _ = client
                                .post(format!("{base_url}/api/v2/torrents/removeCategories"))
                                .header("Cookie", format!("SID={sid}"))
                                .form(&[("categories", cat.as_str())])
                                .send()
                                .await;
                        }
                    }
                }
            }

            let volumes: Vec<i32> = imported.iter().map(|f| f.volume).collect();
            notifications::notify(
                pool.clone(),
                notifications::NotificationEvent::TorrentImportCompleted {
                    library_name: library_name.clone(),
                    series_name: series_name.clone(),
                    imported_count: imported.len(),
                    volumes,
                },
            );

            info!(
                "Torrent import {} done: {} files imported, scan job {} queued",
                torrent_id,
                imported.len(),
                scan_job_id
            );
        }
        Err(e) => {
            let msg = format!("{e:#}");
            warn!("Torrent import {} error: {}", torrent_id, msg);
            sqlx::query(
                "UPDATE torrent_downloads SET status = 'error', error_message = $1, updated_at = NOW() WHERE id = $2",
            )
            .bind(&msg)
            .bind(torrent_id)
            .execute(&pool)
            .await?;

            notifications::notify(
                pool.clone(),
                notifications::NotificationEvent::TorrentImportFailed {
                    library_name: library_name.clone(),
                    series_name: series_name.clone(),
                    error: msg,
                },
            );
        }
    }

    Ok(())
}

use super::import_pipeline::{do_import, remap_downloads_path};

// do_import, filesystem helpers, naming helpers, format dedup, and path remapping
// are now in super::import_pipeline
