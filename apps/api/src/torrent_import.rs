use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{info, trace, warn};
use uuid::Uuid;

use crate::{error::ApiError, prowlarr::extract_volumes_from_title_pub, qbittorrent::{load_qbittorrent_config, qbittorrent_login}, state::AppState};

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
    pub series_name: String,
    pub expected_volumes: Vec<i32>,
    pub qb_hash: Option<String>,
    pub content_path: Option<String>,
    pub status: String,
    pub imported_files: Option<serde_json::Value>,
    pub error_message: Option<String>,
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
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.hash.is_empty() {
        return Err(ApiError::bad_request("hash is required"));
    }

    if !is_torrent_import_enabled(&state.pool).await {
        info!("Torrent import disabled, ignoring notification for hash {}", body.hash);
        return Ok(Json(serde_json::json!({ "ok": true })));
    }

    let row = sqlx::query(
        "SELECT id FROM torrent_downloads WHERE qb_hash = $1 AND status = 'downloading' LIMIT 1",
    )
    .bind(&body.hash)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        info!("Torrent notification for unknown hash {}, ignoring", body.hash);
        return Ok(Json(serde_json::json!({ "ok": true })));
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

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// List recent torrent downloads (admin).
pub async fn list_torrent_downloads(
    State(state): State<AppState>,
) -> Result<Json<Vec<TorrentDownloadDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, library_id, series_name, expected_volumes, qb_hash, content_path, \
         status, imported_files, error_message, created_at, updated_at \
         FROM torrent_downloads ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.pool)
    .await?;

    let dtos = rows
        .into_iter()
        .map(|row| {
            let id: Uuid = row.get("id");
            let library_id: Uuid = row.get("library_id");
            let expected_volumes: Vec<i32> = row.get("expected_volumes");
            let created_at: DateTime<Utc> = row.get("created_at");
            let updated_at: DateTime<Utc> = row.get("updated_at");
            TorrentDownloadDto {
                id: id.to_string(),
                library_id: library_id.to_string(),
                series_name: row.get("series_name"),
                expected_volumes,
                qb_hash: row.get("qb_hash"),
                content_path: row.get("content_path"),
                status: row.get("status"),
                imported_files: row.get("imported_files"),
                error_message: row.get("error_message"),
                created_at: created_at.to_rfc3339(),
                updated_at: updated_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(dtos))
}

// ─── Background poller ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct QbTorrentInfo {
    hash: String,
    state: String,
    content_path: Option<String>,
    save_path: Option<String>,
    name: Option<String>,
}

/// Completed states in qBittorrent: torrent is fully downloaded and seeding.
const QB_COMPLETED_STATES: &[&str] = &[
    "uploading", "stalledUP", "pausedUP", "queuedUP", "checkingUP", "forcedUP",
];

pub async fn run_torrent_poller(pool: PgPool, interval_seconds: u64) {
    let wait = Duration::from_secs(interval_seconds.max(5));
    loop {
        if let Err(e) = poll_qbittorrent_downloads(&pool).await {
            warn!("[TORRENT_POLLER] {:#}", e);
        }
        tokio::time::sleep(wait).await;
    }
}

async fn poll_qbittorrent_downloads(pool: &PgPool) -> anyhow::Result<()> {
    if !is_torrent_import_enabled(pool).await {
        return Ok(());
    }

    let rows = sqlx::query(
        "SELECT id, qb_hash FROM torrent_downloads WHERE status = 'downloading' AND qb_hash IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        trace!("[TORRENT_POLLER] No active downloads to poll");
        return Ok(());
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

        let row = rows.iter().find(|r| {
            let h: String = r.get("qb_hash");
            h == info.hash
        });
        let Some(row) = row else { continue; };
        let torrent_id: Uuid = row.get("id");

        let updated = sqlx::query(
            "UPDATE torrent_downloads SET status = 'completed', content_path = $1, updated_at = NOW() \
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

    Ok(())
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
        "SELECT library_id, series_name, expected_volumes, content_path \
         FROM torrent_downloads WHERE id = $1",
    )
    .bind(torrent_id)
    .fetch_one(&pool)
    .await?;

    let library_id: Uuid = row.get("library_id");
    let series_name: String = row.get("series_name");
    let expected_volumes: Vec<i32> = row.get("expected_volumes");
    let content_path: Option<String> = row.get("content_path");
    let content_path =
        content_path.ok_or_else(|| anyhow::anyhow!("content_path not set on torrent_download"))?;

    sqlx::query(
        "UPDATE torrent_downloads SET status = 'importing', updated_at = NOW() WHERE id = $1",
    )
    .bind(torrent_id)
    .execute(&pool)
    .await?;

    match do_import(&pool, library_id, &series_name, &expected_volumes, &content_path).await {
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
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
            )
            .bind(job_id)
            .bind(library_id)
            .execute(&pool)
            .await?;

            info!(
                "Torrent import {} done: {} files imported, scan job {} queued",
                torrent_id,
                imported.len(),
                job_id
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
) -> anyhow::Result<Vec<ImportedFile>> {
    let physical_content = remap_downloads_path(content_path);

    // Find the target directory and reference file (latest volume) from existing book_files.
    let ref_row = sqlx::query(
        "SELECT bf.abs_path, b.volume \
         FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         WHERE b.library_id = $1 AND b.series = $2 AND b.volume IS NOT NULL \
         ORDER BY b.volume DESC LIMIT 1",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    let (target_dir, reference) = if let Some(r) = ref_row {
        let abs_path: String = r.get("abs_path");
        let volume: i32 = r.get("volume");
        let physical = remap_libraries_path(&abs_path);
        let parent = std::path::Path::new(&physical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(physical);
        (parent, Some((abs_path, volume)))
    } else {
        // No existing files: create series directory inside library root
        let lib_row = sqlx::query("SELECT root_path FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_one(pool)
            .await?;
        let root_path: String = lib_row.get("root_path");
        let physical_root = remap_libraries_path(&root_path);
        let dir = format!("{}/{}", physical_root.trim_end_matches('/'), series_name);
        (dir, None)
    };

    std::fs::create_dir_all(&target_dir)?;

    let expected_set: std::collections::HashSet<i32> = expected_volumes.iter().copied().collect();
    let mut imported = Vec::new();

    for source_path in collect_book_files(&physical_content)? {
        let filename = std::path::Path::new(&source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let ext = std::path::Path::new(&source_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let matched: Vec<i32> = extract_volumes_from_title_pub(filename)
            .into_iter()
            .filter(|v| expected_set.contains(v))
            .collect();

        if matched.is_empty() {
            continue;
        }

        let target_filename = if matched.len() == 1 {
            // Single volume: apply naming pattern from reference
            let vol = matched[0];
            if let Some((ref ref_path, ref_vol)) = reference {
                build_target_filename(ref_path, ref_vol, vol, ext)
                    .unwrap_or_else(|| default_filename(series_name, vol, ext))
            } else {
                default_filename(series_name, vol, ext)
            }
        } else {
            // Multi-volume pack: keep original filename (scanner handles ranges)
            filename.to_string()
        };

        let dest = format!("{}/{}", target_dir, target_filename);

        if std::path::Path::new(&dest).exists() {
            info!("Skipping {} (already exists at destination)", dest);
            continue;
        }

        move_file(&source_path, &dest)?;
        info!("Imported {:?} → {}", matched, dest);

        imported.push(ImportedFile {
            volume: *matched.iter().min().unwrap(),
            source: source_path.clone(),
            destination: unmap_libraries_path(&dest),
        });
    }

    Ok(imported)
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

fn remap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with("/libraries/") {
            return path.replacen("/libraries", &root, 1);
        }
    }
    path.to_string()
}

fn unmap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with(&root) {
            return path.replacen(&root, "/libraries", 1);
        }
    }
    path.to_string()
}

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
    let new_stem = format!("{}{}{}", &stem[..start], new_digits, &stem[end..]);
    Some(format!("{}.{}", new_stem, target_ext))
}

#[cfg(test)]
mod tests {
    use super::build_target_filename;

    #[test]
    fn simple_t_prefix() {
        // "One Piece - T104.cbz" → replace 104 → 105
        let result = build_target_filename(
            "/libraries/One Piece/One Piece - T104.cbz",
            104,
            105,
            "cbz",
        );
        assert_eq!(result, Some("One Piece - T105.cbz".to_string()));
    }

    #[test]
    fn preserves_leading_zeros() {
        let result = build_target_filename(
            "/libraries/Asterix/Asterix - T01.cbz",
            1,
            2,
            "cbz",
        );
        assert_eq!(result, Some("Asterix - T02.cbz".to_string()));
    }

    #[test]
    fn three_digit_zero_padded() {
        let result = build_target_filename(
            "/libraries/Naruto/Naruto T001.cbz",
            1,
            72,
            "cbz",
        );
        assert_eq!(result, Some("Naruto T072.cbz".to_string()));
    }

    #[test]
    fn different_source_ext() {
        // Source file is cbr, reference is cbz
        let result = build_target_filename(
            "/libraries/DBZ/Dragon Ball - T01.cbz",
            1,
            5,
            "cbr",
        );
        assert_eq!(result, Some("Dragon Ball - T05.cbr".to_string()));
    }

    #[test]
    fn accented_series_name() {
        let result = build_target_filename(
            "/libraries/bd/Astérix - T01.cbz",
            1,
            3,
            "cbz",
        );
        assert_eq!(result, Some("Astérix - T03.cbz".to_string()));
    }

    #[test]
    fn no_match_returns_none() {
        // Volume 5 not present in "Series - T01.cbz" whose reference_volume is 99
        let result = build_target_filename(
            "/libraries/Series/Series - T01.cbz",
            99,
            100,
            "cbz",
        );
        assert_eq!(result, None);
    }

    #[test]
    fn uses_last_occurrence() {
        // "Code 451 - T04.cbz" with reference_volume=4 should replace the "04" not the "4" in 451
        let result = build_target_filename(
            "/libraries/Code 451/Code 451 - T04.cbz",
            4,
            5,
            "cbz",
        );
        assert_eq!(result, Some("Code 451 - T05.cbz".to_string()));
    }
}
