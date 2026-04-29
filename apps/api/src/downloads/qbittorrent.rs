use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct QBittorrentAddRequest {
    pub url: String,
    /// When provided together with `series_name` and `expected_volumes`, tracks the download
    /// in `torrent_downloads` and triggers automatic import on completion.
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    pub series_name: Option<String>,
    pub expected_volumes: Option<Vec<i32>>,
    /// When true, overwrite existing files at destination during import.
    #[serde(default)]
    pub replace_existing: bool,
}

#[derive(Serialize, ToSchema)]
pub struct QBittorrentAddResponse {
    pub success: bool,
    pub message: String,
    /// Set when `library_id` + `series_name` + `expected_volumes` were provided.
    #[schema(value_type = Option<String>)]
    pub torrent_download_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct QBittorrentTestResponse {
    pub success: bool,
    pub message: String,
    pub version: Option<String>,
}

// ─── Config helper ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct QBittorrentConfig {
    url: String,
    username: String,
    password: String,
}

pub(crate) async fn load_qbittorrent_config(
    pool: &sqlx::PgPool,
) -> Result<(String, String, String), ApiError> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'qbittorrent'")
        .fetch_optional(pool)
        .await?;

    let row = row.ok_or_else(|| ApiError::bad_request("qBittorrent is not configured"))?;
    let value: serde_json::Value = row.get("value");
    let config: QBittorrentConfig = serde_json::from_value(value)
        .map_err(|e| ApiError::internal(format!("invalid qbittorrent config: {e}")))?;

    if config.url.is_empty() || config.username.is_empty() {
        return Err(ApiError::bad_request(
            "qBittorrent URL and username must be configured in settings",
        ));
    }

    let url = config.url.trim_end_matches('/').to_string();
    Ok((url, config.username, config.password))
}

// ─── Login helper ───────────────────────────────────────────────────────────

pub(crate) async fn qbittorrent_login(
    client: &reqwest::Client,
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<String, ApiError> {
    let resp = client
        .post(format!("{base_url}/api/v2/auth/login"))
        .form(&[("username", username), ("password", password)])
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("qBittorrent login request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::internal(format!(
            "qBittorrent login failed ({status}): {text}"
        )));
    }

    // Extract SID from Set-Cookie header
    let cookie_header = resp
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let sid = cookie_header
        .split(';')
        .next()
        .and_then(|s| s.strip_prefix("SID="))
        .ok_or_else(|| ApiError::internal("Failed to get SID cookie from qBittorrent"))?
        .to_string();

    Ok(sid)
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Add a torrent to qBittorrent
#[utoipa::path(
    post,
    path = "/qbittorrent/add",
    tag = "qbittorrent",
    request_body = QBittorrentAddRequest,
    responses(
        (status = 200, body = QBittorrentAddResponse),
        (status = 400, description = "Bad request or qBittorrent not configured"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "qBittorrent connection error"),
    ),
    security(("Bearer" = []))
)]
pub async fn add_torrent(
    State(state): State<AppState>,
    Json(body): Json<QBittorrentAddRequest>,
) -> Result<Json<QBittorrentAddResponse>, ApiError> {
    if body.url.is_empty() {
        return Err(ApiError::bad_request("url is required"));
    }

    // Always track the torrent. Infer missing fields from the URL/title if needed.
    let inferred_series_name = body.series_name.clone().or_else(|| {
        // Extract series name from the URL filename param or the URL itself
        let url = &body.url;
        url.split("file=").nth(1)
            .map(|f| {
                let encoded = f.split('&').next().unwrap_or(f);
                // Simple percent-decode + '+' to space
                let decoded = encoded.replace('+', " ");
                let mut result = String::new();
                let mut chars = decoded.chars();
                while let Some(c) = chars.next() {
                    if c == '%' {
                        let hex: String = chars.by_ref().take(2).collect();
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                        }
                    } else {
                        result.push(c);
                    }
                }
                result
            })
            .map(|f| crate::integrations::discovery::extract_series_name_from_torrent(&f))
            .filter(|s| !s.is_empty())
    });
    let inferred_library_id = match body.library_id {
        Some(id) => Some(id),
        None => {
            // Use the first library as fallback
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM libraries ORDER BY created_at LIMIT 1")
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten()
        }
    };
    let is_managed = inferred_library_id.is_some() && inferred_series_name.is_some();

    tracing::info!("[QBITTORRENT] Add torrent request: url={}, managed={is_managed}", body.url);

    let (base_url, username, password) = load_qbittorrent_config(&state.pool).await.inspect_err(|e| {
        tracing::error!("[QBITTORRENT] Failed to load config: {}", e.message);
    })?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| {
            tracing::error!("[QBITTORRENT] Failed to build HTTP client: {e}");
            ApiError::internal(format!("failed to build HTTP client: {e}"))
        })?;

    // Resolve the URL: if it's an HTTP(S) link (e.g. Prowlarr proxy), follow redirects
    // and download the .torrent file ourselves, since qBittorrent may not handle redirects.
    // If the final URL is a magnet, use it directly.
    let resolved = resolve_torrent_url(&body.url).await.map_err(|e| {
        tracing::error!("[QBITTORRENT] Failed to resolve torrent URL: {e}");
        ApiError::internal(format!("Failed to resolve torrent URL: {e}"))
    })?;
    let resolved_magnet_url; // keep owned String alive for borrowing
    let torrent_bytes: Option<Vec<u8>>;
    match resolved {
        ResolvedTorrent::Magnet(m) => {
            tracing::info!("[QBITTORRENT] Resolved to magnet link");
            resolved_magnet_url = Some(m);
            torrent_bytes = None;
        }
        ResolvedTorrent::TorrentFile(bytes) => {
            tracing::info!("[QBITTORRENT] Resolved to .torrent file ({} bytes)", bytes.len());
            resolved_magnet_url = None;
            torrent_bytes = Some(bytes);
        }
    }

    let sid = qbittorrent_login(&client, &base_url, &username, &password).await.inspect_err(|e| {
        tracing::error!("[QBITTORRENT] Login failed: {}", e.message);
    })?;

    // Pre-generate the download ID; use a unique category per download so we can
    // reliably match the torrent back (tags/savepath are unreliable on qBittorrent 4.x).
    let download_id = if is_managed { Some(Uuid::new_v4()) } else { None };
    let category = download_id.as_ref().map(|id| format!("sl-{id}"));

    // Create the category in qBittorrent before adding the torrent
    if let Some(ref cat) = category {
        let _ = client
            .post(format!("{base_url}/api/v2/torrents/createCategory"))
            .header("Cookie", format!("SID={sid}"))
            .form(&[("category", cat.as_str()), ("savePath", "/downloads")])
            .send()
            .await;
    }

    let savepath = "/downloads";

    let resp = if let Some(ref torrent_data) = torrent_bytes {
        // Upload .torrent file via multipart
        let mut form = reqwest::multipart::Form::new()
            .part("torrents", reqwest::multipart::Part::bytes(torrent_data.clone())
                .file_name("torrent.torrent")
                .mime_str("application/x-bittorrent")
                .unwrap());
        if is_managed {
            form = form.text("savepath", savepath.to_string());
            if let Some(ref cat) = category {
                form = form.text("category", cat.clone());
            }
        }
        client
            .post(format!("{base_url}/api/v2/torrents/add"))
            .header("Cookie", format!("SID={sid}"))
            .multipart(form)
            .send()
            .await
    } else {
        // Pass magnet URL or original URL directly
        let url_to_send = resolved_magnet_url.as_deref().unwrap_or(&body.url);
        let mut form_params: Vec<(&str, &str)> = vec![("urls", url_to_send)];
        if is_managed {
            form_params.push(("savepath", savepath));
            if let Some(ref cat) = category {
                form_params.push(("category", cat));
            }
        }
        client
            .post(format!("{base_url}/api/v2/torrents/add"))
            .header("Cookie", format!("SID={sid}"))
            .form(&form_params)
            .send()
            .await
    }.map_err(|e| {
        tracing::error!("[QBITTORRENT] Add torrent request failed: {e}");
        ApiError::internal(format!("qBittorrent add request failed: {e}"))
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        tracing::error!("[QBITTORRENT] Add torrent failed — status={status}, body={text}");
        return Ok(Json(QBittorrentAddResponse {
            success: false,
            message: format!("qBittorrent returned {status}: {text}"),
            torrent_download_id: None,
        }));
    }

    // If managed download: record in torrent_downloads
    let torrent_download_id = if is_managed {
        let library_id = inferred_library_id.unwrap();
        let series_name = inferred_series_name.as_deref().unwrap();
        let expected_volumes = body.expected_volumes.as_deref().unwrap_or(&[]);

        // Try to resolve hash: first from magnet, then by category in qBittorrent
        let mut qb_hash = extract_magnet_hash(&body.url);
        if qb_hash.is_none() {
            if let Some(ref cat) = category {
                // Magnet links take time to be resolved by qBittorrent.
                // Retry with increasing delays to give qBittorrent time to register the torrent.
                for delay_secs in [2, 3, 5] {
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                    qb_hash = resolve_hash_by_category(&client, &base_url, &sid, cat).await;
                    if qb_hash.is_some() {
                        break;
                    }
                }
            }

            // If still no hash, the torrent might already exist in qBittorrent (duplicate).
            // Try to find it by searching all torrents for a matching category save_path.
            if qb_hash.is_none() {
                tracing::warn!("[QBITTORRENT] No hash resolved by category, searching all torrents for duplicate");
                if let Ok(all_resp) = client
                    .get(format!("{base_url}/api/v2/torrents/info"))
                    .header("Cookie", format!("SID={sid}"))
                    .send()
                    .await
                {
                    if let Ok(all_torrents) = all_resp.json::<Vec<serde_json::Value>>().await {
                        // Find a torrent whose save_path contains our category directory
                        if let Some(ref cat) = category {
                            for t in &all_torrents {
                                let t_cat = t.get("category").and_then(|c| c.as_str()).unwrap_or("");
                                let t_save = t.get("save_path").and_then(|s| s.as_str()).unwrap_or("");
                                let t_content = t.get("content_path").and_then(|s| s.as_str()).unwrap_or("");
                                if t_cat == cat.as_str() || t_save.contains(cat.as_str()) || t_content.contains(cat.as_str()) {
                                    qb_hash = t.get("hash").and_then(|h| h.as_str()).map(String::from);
                                    if qb_hash.is_some() {
                                        tracing::info!("[QBITTORRENT] Found duplicate torrent by save_path: hash={}", qb_hash.as_deref().unwrap_or("?"));
                                        break;
                                    }
                                }
                            }
                            // If still not found, the torrent exists under a different category.
                            // Try matching by magnet hash if available from the URL.
                            if qb_hash.is_none() {
                                if let Some(magnet_hash) = extract_magnet_hash(&body.url).or_else(|| resolved_magnet_url.as_deref().and_then(extract_magnet_hash)) {
                                    let magnet_lower = magnet_hash.to_lowercase();
                                    for t in &all_torrents {
                                        if let Some(h) = t.get("hash").and_then(|h| h.as_str()) {
                                            if h.to_lowercase() == magnet_lower {
                                                qb_hash = Some(h.to_string());
                                                // Get the real content_path from the existing torrent
                                                let existing_content = t.get("content_path").and_then(|c| c.as_str()).unwrap_or("").to_string();
                                                tracing::info!("[QBITTORRENT] Found existing torrent by magnet hash {h}, content_path={existing_content}");

                                                // If the torrent is already completed, check if content still exists
                                                let t_state = t.get("state").and_then(|s| s.as_str()).unwrap_or("");
                                                let t_progress = t.get("progress").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                                if t_progress >= 1.0 || super::torrent_import::QB_COMPLETED_STATES.contains(&t_state) {
                                                    let content_exists = !existing_content.is_empty()
                                                        && tokio::fs::metadata(&existing_content).await.is_ok();

                                                    if !content_exists {
                                                        // Old torrent is a stale residue — content was already cleaned up.
                                                        // Remove it from qBittorrent and let the new torrent be added normally.
                                                        tracing::warn!("[QBITTORRENT] Duplicate torrent {h} content no longer exists at {existing_content}, removing stale torrent");
                                                        let _ = client
                                                            .post(format!("{base_url}/api/v2/torrents/delete"))
                                                            .header("Cookie", format!("SID={sid}"))
                                                            .form(&[("hashes", h.to_string()), ("deleteFiles", "false".to_string())])
                                                            .send()
                                                            .await;
                                                        qb_hash = None;
                                                        break;
                                                    }

                                                    // Content exists — launch import immediately
                                                    let pool2 = state.pool.clone();
                                                    let tid = download_id.unwrap();
                                                    let lib_id = library_id;
                                                    let sn = series_name;
                                                    let ev = expected_volumes.to_vec();
                                                    let replace = body.replace_existing;
                                                    let cp = existing_content.clone();

                                                    sqlx::query(
                                                        "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, qb_hash, content_path, status, replace_existing, progress) \
                                                         VALUES ($1, $2, $3, $4, $5, $6, 'completed', $7, 1)",
                                                    )
                                                    .bind(tid)
                                                    .bind(lib_id)
                                                    .bind(sn)
                                                    .bind(&ev)
                                                    .bind(h)
                                                    .bind(&cp)
                                                    .bind(replace)
                                                    .execute(&state.pool)
                                                    .await
                                                    .ok();

                                                    tracing::info!("[QBITTORRENT] Duplicate torrent already completed, launching import immediately for {tid}");

                                                    tokio::spawn(async move {
                                                        if let Err(e) = super::torrent_import::process_torrent_import(pool2, tid).await {
                                                            tracing::warn!("Import {tid} failed: {e:#}");
                                                        }
                                                    });

                                                    return Ok(Json(QBittorrentAddResponse {
                                                        success: true,
                                                        message: "Duplicate torrent detected, import launched".to_string(),
                                                        torrent_download_id: Some(tid),
                                                    }));
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let id = download_id.unwrap();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, qb_hash, replace_existing) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(library_id)
        .bind(series_name)
        .bind(expected_volumes)
        .bind(qb_hash.as_deref())
        .bind(body.replace_existing)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("[QBITTORRENT] Failed to insert torrent_downloads row: {e}");
            ApiError::from(e)
        })?;

        tracing::info!("[QBITTORRENT] Created torrent download {id} for {series_name}, qb_hash={qb_hash:?}");

        Some(id)
    } else {
        None
    };

    Ok(Json(QBittorrentAddResponse {
        success: true,
        message: "Torrent added to qBittorrent".to_string(),
        torrent_download_id,
    }))
}

// ─── URL resolution ─────────────────────────────────────────────────────────

enum ResolvedTorrent {
    Magnet(String),
    TorrentFile(Vec<u8>),
}

/// Resolve a torrent URL: if it's already a magnet link, return as-is.
/// Otherwise follow HTTP redirects. If the final URL is a magnet, return it.
/// If the response is a .torrent file, return the bytes.
async fn resolve_torrent_url(
    url: &str,
) -> Result<ResolvedTorrent, String> {
    // Already a magnet link — nothing to resolve
    if url.starts_with("magnet:") {
        return Ok(ResolvedTorrent::Magnet(url.to_string()));
    }

    // Build a client that does NOT follow redirects so we can inspect Location headers
    let no_redirect_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Stripstream-Librarian")
        .build()
        .map_err(|e| format!("failed to build redirect client: {e}"))?;

    let mut current_url = url.to_string();
    for _ in 0..10 {
        let resp = no_redirect_client
            .get(&current_url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed for {current_url}: {e}"))?;

        let status = resp.status();
        if status.is_redirection() {
            if let Some(location) = resp.headers().get("location").and_then(|v| v.to_str().ok()) {
                let location = location.to_string();
                if location.starts_with("magnet:") {
                    tracing::info!("[QBITTORRENT] URL redirected to magnet link");
                    return Ok(ResolvedTorrent::Magnet(location));
                }
                tracing::debug!("[QBITTORRENT] Following redirect: {status} -> {location}");
                current_url = location;
                continue;
            }
            return Err(format!("redirect {status} without Location header"));
        }

        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("HTTP {status}: {text}"));
        }

        // Successful response — download the .torrent file
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("failed to read response body: {e}"))?;

        if bytes.is_empty() {
            return Err("empty response body".to_string());
        }

        return Ok(ResolvedTorrent::TorrentFile(bytes.to_vec()));
    }

    Err("too many redirects".to_string())
}

/// Extract the info-hash from a magnet link (lowercased hex).
/// magnet:?xt=urn:btih:HASH...
/// Handles both hex (40 chars) and base32 (32 chars) encoded hashes.
fn extract_magnet_hash(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    let marker = "urn:btih:";
    let start = lower.find(marker)? + marker.len();
    let hash_part = &lower[start..];
    let end = hash_part
        .find(|c: char| !c.is_alphanumeric())
        .unwrap_or(hash_part.len());
    let hash = &hash_part[..end];
    if hash.is_empty() {
        return None;
    }
    // 40-char hex hash: use as-is
    if hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(hash.to_string());
    }
    // 32-char base32 hash: decode to hex
    if hash.len() == 32 {
        if let Some(hex) = base32_to_hex(hash) {
            return Some(hex);
        }
    }
    // Fallback: return as-is (may not match qBittorrent)
    Some(hash.to_string())
}

/// Decode a base32-encoded string to a lowercase hex string.
fn base32_to_hex(input: &str) -> Option<String> {
    let input = input.to_uppercase();
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits: u64 = 0;
    let mut bit_count = 0u32;
    let mut bytes = Vec::with_capacity(20);

    for ch in input.bytes() {
        let val = alphabet.iter().position(|&c| c == ch)? as u64;
        bits = (bits << 5) | val;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            bytes.push((bits >> bit_count) as u8);
            bits &= (1u64 << bit_count) - 1;
        }
    }

    if bytes.len() != 20 {
        return None;
    }
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Torrent entry from qBittorrent API.
#[derive(Deserialize, Clone)]
struct QbTorrentEntry {
    hash: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    save_path: String,
    #[serde(default)]
    category: String,
}

/// Resolve the hash of a torrent by its unique category in qBittorrent.
/// Each managed torrent gets category `sl-{download_id}` at add time,
/// so we can reliably find it even when multiple torrents are added concurrently.
pub(crate) async fn resolve_hash_by_category(
    client: &reqwest::Client,
    base_url: &str,
    sid: &str,
    category: &str,
) -> Option<String> {
    let resp = client
        .get(format!("{base_url}/api/v2/torrents/info"))
        .query(&[("category", category)])
        .header("Cookie", format!("SID={sid}"))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        tracing::warn!("[QBITTORRENT] Failed to query torrents by category {category}");
        return None;
    }

    let torrents: Vec<QbTorrentEntry> = resp.json().await.unwrap_or_default();
    if torrents.len() == 1 {
        tracing::info!("[QBITTORRENT] Resolved hash {} via category {category} ({})", torrents[0].hash, torrents[0].name);
        return Some(torrents[0].hash.clone());
    }

    if !torrents.is_empty() {
        tracing::warn!("[QBITTORRENT] Multiple torrents with category {category}, expected 1");
        return None;
    }

    // Fallback: search ALL torrents and match by save_path containing the download ID
    // This handles cases where qBittorrent didn't associate the category (magnet links)
    let download_id = category.strip_prefix("sl-").unwrap_or(category);
    let all_resp = client
        .get(format!("{base_url}/api/v2/torrents/info"))
        .header("Cookie", format!("SID={sid}"))
        .send()
        .await
        .ok();

    if let Some(all_resp) = all_resp {
        if all_resp.status().is_success() {
            let all_torrents: Vec<QbTorrentEntry> = all_resp.json().await.unwrap_or_default();
            // Look for a torrent whose save_path contains our download ID
            let matched: Vec<&QbTorrentEntry> = all_torrents
                .iter()
                .filter(|t| t.save_path.contains(download_id) || t.category.contains(download_id))
                .collect();
            if matched.len() == 1 {
                tracing::info!(
                    "[QBITTORRENT] Resolved hash {} via save_path fallback for {category} ({})",
                    matched[0].hash, matched[0].name
                );
                // Also fix the category on the torrent for future lookups
                let _ = client
                    .post(format!("{base_url}/api/v2/torrents/setCategory"))
                    .header("Cookie", format!("SID={sid}"))
                    .form(&[("hashes", matched[0].hash.as_str()), ("category", category)])
                    .send()
                    .await;
                return Some(matched[0].hash.clone());
            }
        }
    }

    tracing::warn!("[QBITTORRENT] No torrent found with category {category} (even with save_path fallback)");
    None
}

/// Test connection to qBittorrent
#[utoipa::path(
    get,
    path = "/qbittorrent/test",
    tag = "qbittorrent",
    responses(
        (status = 200, body = QBittorrentTestResponse),
        (status = 400, description = "qBittorrent not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn test_qbittorrent(
    State(state): State<AppState>,
) -> Result<Json<QBittorrentTestResponse>, ApiError> {
    let (base_url, username, password) = load_qbittorrent_config(&state.pool).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let sid = match qbittorrent_login(&client, &base_url, &username, &password).await {
        Ok(sid) => sid,
        Err(e) => {
            return Ok(Json(QBittorrentTestResponse {
                success: false,
                message: format!("Login failed: {}", e.message),
                version: None,
            }));
        }
    };

    let resp = client
        .get(format!("{base_url}/api/v2/app/version"))
        .header("Cookie", format!("SID={sid}"))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let version = r.text().await.unwrap_or_default();
            Ok(Json(QBittorrentTestResponse {
                success: true,
                message: format!("Connected successfully ({})", version.trim()),
                version: Some(version.trim().to_string()),
            }))
        }
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            Ok(Json(QBittorrentTestResponse {
                success: false,
                message: format!("qBittorrent returned {status}: {text}"),
                version: None,
            }))
        }
        Err(e) => Ok(Json(QBittorrentTestResponse {
            success: false,
            message: format!("Connection failed: {e}"),
            version: None,
        })),
    }
}
