use axum::{extract::{Path, State}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{info, trace, warn};
use uuid::Uuid;

use crate::{error::ApiError, metadata, state::AppState};
use super::qbittorrent::{load_qbittorrent_config, qbittorrent_login, resolve_hash_by_category};
use parsers::extract_volumes;

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
struct ImportedFile {
    volume: i32,
    source: String,
    destination: String,
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

async fn do_import(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    expected_volumes: &[i32],
    content_path: &str,
    replace_existing: bool,
) -> anyhow::Result<Vec<ImportedFile>> {
    let physical_content = remap_downloads_path(content_path);

    // Find the target directory and a naming reference from existing book_files.
    // First find ANY existing book to determine the target directory, then pick a
    // reference file (preferring one outside expected_volumes for naming consistency).
    let any_row = sqlx::query(
        "SELECT bf.abs_path, b.volume \
         FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) \
           AND b.volume IS NOT NULL \
         ORDER BY b.volume DESC LIMIT 1",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    let (target_dir, reference) = if let Some(r) = any_row {
        let abs_path: String = r.get("abs_path");
        let volume: i32 = r.get("volume");
        let physical = remap_libraries_path(&abs_path);
        let parent = std::path::Path::new(&physical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(physical);
        info!("[IMPORT] DB reference found: {} (volume {}), target_dir={}", abs_path, volume, parent);
        (parent, Some((abs_path, volume)))
    } else {
        // No existing files in DB: look for an existing directory (case-insensitive)
        // inside the library root, then fall back to creating one.
        info!("[IMPORT] No DB reference for series '{}' in library {}", series_name, library_id);
        let lib_row = sqlx::query("SELECT root_path FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_one(pool)
            .await?;
        let root_path: String = lib_row.get("root_path");
        let physical_root = remap_libraries_path(&root_path);
        let dir = find_existing_series_dir(&physical_root, series_name)
            .unwrap_or_else(|| format!("{}/{}", physical_root.trim_end_matches('/'), series_name));
        info!("[IMPORT] Target directory: {}", dir);
        (dir, None)
    };

    std::fs::create_dir_all(&target_dir)?;

    let mut expected_set: std::collections::HashSet<i32> = expected_volumes.iter().copied().collect();

    // If DB didn't give us a reference, try to find one from existing files on disk
    let reference = if reference.is_some() {
        reference
    } else {
        info!("[IMPORT] Trying disk fallback in {}", target_dir);
        let disk_ref = find_reference_from_disk(&target_dir, &expected_set);
        if disk_ref.is_none() {
            info!("[IMPORT] No disk reference found either, using default naming");
        }
        disk_ref
    };

    info!("[IMPORT] Final reference: {:?}", reference);
    info!("[IMPORT] Expected volumes (from download): {:?}", expected_set);
    info!("[IMPORT] Physical content path: {}", physical_content);

    // Collect all candidate files, then deduplicate by volume keeping the best format.
    // Priority: cbz > cbr > pdf > epub
    let all_source_files = collect_book_files(&physical_content)?;
    info!("[IMPORT] Found {} source files: {:?}", all_source_files.len(), all_source_files);
    for f in &all_source_files {
        let fname = std::path::Path::new(f).file_name().and_then(|n| n.to_str()).unwrap_or("");
        let extracted = extract_volumes(fname);
        info!("[IMPORT]   '{}' => extracted volumes: {:?}", fname, extracted);
    }

    // Expand expected_set: also import volumes found in the torrent that are missing
    // from the library (not just the volumes originally expected by the download detection)
    if !replace_existing && !expected_set.is_empty() {
        let all_torrent_volumes: std::collections::HashSet<i32> = all_source_files.iter()
            .flat_map(|f| {
                let fname = std::path::Path::new(f).file_name().and_then(|n| n.to_str()).unwrap_or("");
                extract_volumes(fname)
            })
            .collect();

        if !all_torrent_volumes.is_empty() {
            let existing_volumes: Vec<i32> = sqlx::query_scalar(
                "SELECT DISTINCT b.volume FROM books b \
                 LEFT JOIN series s ON s.id = b.series_id \
                 WHERE b.library_id = $1 AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) AND b.volume IS NOT NULL"
            )
            .bind(library_id)
            .bind(series_name)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let existing_set: std::collections::HashSet<i32> = existing_volumes.into_iter().collect();
            let missing_in_library: Vec<i32> = all_torrent_volumes.iter()
                .filter(|v| !existing_set.contains(v))
                .copied()
                .collect();

            if !missing_in_library.is_empty() {
                info!("[IMPORT] Expanding expected_set with {} additional missing volumes: {:?}", missing_in_library.len(), missing_in_library);
                expected_set.extend(missing_in_library);
            }
        }
    }
    info!("[IMPORT] Final expected volumes: {:?}", expected_set);

    // In replace mode, don't filter by expected volumes — import all files
    let dedup_set = if replace_existing { std::collections::HashSet::new() } else { expected_set.clone() };
    let source_files = deduplicate_by_format(&all_source_files, &dedup_set);
    info!("[IMPORT] After dedup: {} files kept", source_files.len());

    let mut imported = Vec::new();
    let mut used_destinations: std::collections::HashSet<String> = std::collections::HashSet::new();

    for source_path in &source_files {
        let filename = std::path::Path::new(&source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let ext = std::path::Path::new(&source_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let all_extracted = extract_volumes(filename);
        let matched: Vec<i32> = if expected_set.is_empty() || replace_existing {
            all_extracted.clone() // No filter — import everything (empty set or replace mode)
        } else {
            all_extracted.iter().copied().filter(|v| expected_set.contains(v)).collect()
        };

        if matched.is_empty() && !expected_set.is_empty() && !replace_existing {
            info!("[IMPORT] Skipping '{}' (extracted volumes {:?}, none in expected set)", filename, all_extracted);
            continue;
        }
        if matched.is_empty() && expected_set.is_empty() && all_extracted.is_empty() {
            // No volume detected and no expected set — keep the file as-is (e.g. HS)
            // Skip for now, will be handled by a future HS import feature
            info!("[IMPORT] Skipping '{}' (no volume detected, no expected set)", filename);
            continue;
        }

        let target_filename = if matched.len() == 1 {
            // Single volume: apply naming pattern from reference
            let vol = matched[0];
            let generated = if let Some((ref ref_path, ref_vol)) = reference {
                let built = build_target_filename(ref_path, ref_vol, vol, ext);
                info!("[IMPORT] build_target_filename(ref={}, ref_vol={}, new_vol={}, ext={}) => {:?} (source='{}')",
                    ref_path, ref_vol, vol, ext, built, filename);
                built.unwrap_or_else(|| default_filename(series_name, vol, ext))
            } else {
                info!("[IMPORT] No reference, using default_filename for vol {} (source='{}')", vol, filename);
                default_filename(series_name, vol, ext)
            };

            // If this destination was already used in this batch, keep original filename
            if used_destinations.contains(&generated) {
                info!("[IMPORT] Destination '{}' already used in this batch, keeping original filename '{}'", generated, filename);
                filename.to_string()
            } else {
                generated
            }
        } else {
            // Multi-volume pack: keep original filename (scanner handles ranges)
            filename.to_string()
        };

        let dest = format!("{}/{}", target_dir, target_filename);

        if std::path::Path::new(&dest).exists() && !replace_existing {
            info!("[IMPORT] Skipping '{}' → '{}' (already exists at destination)", filename, dest);
            continue;
        }

        move_file(&source_path, &dest)?;
        used_destinations.insert(target_filename);
        info!("[IMPORT] Imported '{}' [{:?}] → {}", filename, matched, dest);

        imported.push(ImportedFile {
            volume: *matched.iter().min().unwrap(),
            source: source_path.clone(),
            destination: unmap_libraries_path(&dest),
        });
    }

    // Sanity check: warn if many source files collapsed into few volumes
    // (symptom of a volume extraction bug)
    let source_count = collect_book_files(&physical_content).map(|f| f.len()).unwrap_or(0);
    let unique_volumes: std::collections::HashSet<i32> = imported.iter().map(|f| f.volume).collect();
    if source_count > 5 && unique_volumes.len() > 0 && source_count > unique_volumes.len() * 3 {
        warn!(
            "[IMPORT] Suspicious: {} source files mapped to only {} unique volumes ({:?}). \
             Possible volume extraction issue for series '{}'",
            source_count, unique_volumes.len(),
            {
                let mut v: Vec<i32> = unique_volumes.into_iter().collect();
                v.sort();
                v
            },
            series_name,
        );
    }

    Ok(imported)
}

// ─── Directory matching ───────────────────────────────────────────────────────

/// Find an existing directory in `root` whose name matches `series_name`
/// case-insensitively and accent-insensitively (e.g. "les géants" matches "les geants").
fn find_existing_series_dir(root: &str, series_name: &str) -> Option<String> {
    let target_norm = strip_accents(&series_name.to_lowercase());
    let entries = std::fs::read_dir(root).ok()?;
    let mut best: Option<(String, bool)> = None; // (path, is_exact_case_match)
    for entry in entries.flatten() {
        if !entry.file_type().ok().map_or(false, |t| t.is_dir()) {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let name_lower = name_str.to_lowercase();
        let name_norm = strip_accents(&name_lower);
        if name_norm == target_norm {
            let path = entry.path().to_string_lossy().into_owned();
            let exact = name_lower == series_name.to_lowercase();
            info!("[IMPORT] Found existing directory (normalized match): {} (exact={})", path, exact);
            // Prefer exact case match over accent-stripped match
            if exact || best.is_none() {
                best = Some((path, exact));
            }
        }
    }
    best.map(|(p, _)| p)
}

/// Remove diacritical marks from a string (é→e, à→a, ü→u, etc.)
fn strip_accents(s: &str) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        // Decompose the character and skip combining marks (U+0300..U+036F)
        // Map common accented chars to their base letter
        let _ = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => result.write_char('a'),
            'è' | 'é' | 'ê' | 'ë' => result.write_char('e'),
            'ì' | 'í' | 'î' | 'ï' => result.write_char('i'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => result.write_char('o'),
            'ù' | 'ú' | 'û' | 'ü' => result.write_char('u'),
            'ý' | 'ÿ' => result.write_char('y'),
            'ñ' => result.write_char('n'),
            'ç' => result.write_char('c'),
            'æ' => result.write_str("ae"),
            'œ' => result.write_str("oe"),
            _ => result.write_char(c),
        };
    }
    result
}

// ─── Format deduplication ─────────────────────────────────────────────────────

/// When a download contains the same volume in multiple formats (e.g. T01.cbz and T01.pdf),
/// keep only the best format per volume. Priority: cbz > cbr > pdf > epub.
fn format_priority(ext: &str) -> u8 {
    match ext.to_ascii_lowercase().as_str() {
        "cbz" => 0,
        "cbr" => 1,
        "pdf" => 2,
        "epub" => 3,
        _ => 4,
    }
}

fn deduplicate_by_format(
    files: &[String],
    expected_set: &std::collections::HashSet<i32>,
) -> Vec<String> {
    // Map: volume -> (priority, file_path)
    let mut best_per_vol: std::collections::HashMap<i32, (u8, &str)> = std::collections::HashMap::new();
    let mut multi_volume_files: Vec<&str> = Vec::new();

    for path in files {
        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let all_volumes = extract_volumes(filename);
        // If expected_set is empty, keep all files (no filtering)
        let volumes: Vec<i32> = if expected_set.is_empty() {
            all_volumes
        } else {
            all_volumes.into_iter().filter(|v| expected_set.contains(v)).collect()
        };

        if volumes.is_empty() && !expected_set.is_empty() {
            continue;
        }
        // For files with no extracted volume and empty expected_set, keep them
        if volumes.is_empty() && expected_set.is_empty() {
            multi_volume_files.push(path);
            continue;
        }

        if volumes.len() > 1 {
            // Multi-volume packs are always kept (no dedup possible)
            multi_volume_files.push(path);
            continue;
        }

        let vol = volumes[0];
        let prio = format_priority(ext);
        if best_per_vol.get(&vol).map_or(true, |(p, _)| prio < *p) {
            best_per_vol.insert(vol, (prio, path));
        }
    }

    let mut result: Vec<String> = best_per_vol
        .into_values()
        .map(|(_, path)| path.to_string())
        .collect();
    result.extend(multi_volume_files.into_iter().map(|s| s.to_string()));
    result
}

// ─── Reference from disk ──────────────────────────────────────────────────────

/// Scan a directory for book files and pick the one with the highest extracted volume
/// as a naming reference, excluding certain volumes. Returns (abs_path, volume).
fn find_reference_from_disk(dir: &str, exclude_volumes: &std::collections::HashSet<i32>) -> Option<(String, i32)> {
    let extensions = ["cbz", "cbr", "pdf", "epub"];
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(String, i32)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
            continue;
        }
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let volumes = extract_volumes(filename);
        if let Some(&vol) = volumes.iter().max() {
            if exclude_volumes.contains(&vol) {
                continue;
            }
            if best.as_ref().map_or(true, |(_, v)| vol > *v) {
                best = Some((path.to_string_lossy().into_owned(), vol));
            }
        }
    }

    if let Some((ref path, vol)) = best {
        info!("[IMPORT] Found disk reference: {} (volume {})", path, vol);
    }
    best
}

// ─── Filesystem helpers ───────────────────────────────────────────────────────

fn collect_book_files(root: &str) -> anyhow::Result<Vec<String>> {
    let extensions = ["cbz", "cbr", "pdf", "epub"];
    let mut files = Vec::new();
    collect_recursive(root, &extensions, &mut files)?;
    Ok(files)
}

fn collect_recursive(path: &str, exts: &[&str], out: &mut Vec<String>) -> anyhow::Result<()> {
    let p = std::path::Path::new(path);
    if p.is_file() {
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if exts.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                out.push(path.to_string());
            }
        }
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            collect_recursive(&child, exts, out)?;
        } else if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if exts.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                out.push(child);
            }
        }
    }
    Ok(())
}

fn move_file(src: &str, dst: &str) -> anyhow::Result<()> {
    if std::fs::rename(src, dst).is_err() {
        // Cross-device link: copy then remove
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
    }
    Ok(())
}

// ─── Path remapping ───────────────────────────────────────────────────────────

fn remap_downloads_path(path: &str) -> String {
    if let Ok(root) = std::env::var("DOWNLOADS_PATH") {
        if path.starts_with("/downloads") {
            return path.replacen("/downloads", &root, 1);
        }
    }
    path.to_string()
}

use stripstream_core::paths::{remap_libraries_path, unmap_libraries_path};

// ─── Naming helpers ───────────────────────────────────────────────────────────

fn default_filename(series_name: &str, volume: i32, ext: &str) -> String {
    format!("{} - T{:02}.{}", series_name, volume, ext)
}

/// Infer the target filename for `new_volume` by reusing the naming pattern from
/// `reference_abs_path` (which stores `reference_volume`).
///
/// Strategy: find the last digit run in the reference stem that parses to `reference_volume`,
/// replace it with `new_volume` formatted to the same width (preserves leading zeros).
///
/// Example:
///   reference = "/libraries/bd/One Piece/One Piece - T104.cbz", reference_volume = 104
///   new_volume = 105, source_ext = "cbz"
///   → "One Piece - T105.cbz"
fn build_target_filename(
    reference_abs_path: &str,
    reference_volume: i32,
    new_volume: i32,
    source_ext: &str,
) -> Option<String> {
    let path = std::path::Path::new(reference_abs_path);
    let stem = path.file_stem()?.to_str()?;
    let ref_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("cbz");
    let target_ext = if source_ext.is_empty() { ref_ext } else { source_ext };

    // Iterate over raw bytes to find ASCII digit runs (safe: continuation bytes of
    // multi-byte UTF-8 sequences are never in the ASCII digit range 0x30–0x39).
    let bytes = stem.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut last_match: Option<(usize, usize)> = None; // byte offsets into `stem`

    while i < len {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let digit_str = &stem[start..i]; // valid UTF-8: all ASCII digits
            if let Ok(n) = digit_str.parse::<i32>() {
                if n == reference_volume {
                    last_match = Some((start, i));
                }
            }
        } else {
            i += 1;
        }
    }

    let (start, end) = last_match?;
    let digit_width = end - start;
    let new_digits = format!("{:0>width$}", new_volume, width = digit_width);
    // Truncate after the volume number (remove suffixes like ".FR-NoFace696")
    let new_stem = format!("{}{}", &stem[..start], new_digits);
    Some(format!("{}.{}", new_stem, target_ext))
}

#[cfg(test)]
#[path = "torrent_import_tests.rs"]
mod tests;
