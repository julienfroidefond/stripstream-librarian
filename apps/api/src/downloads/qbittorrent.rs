use std::net::{IpAddr, SocketAddr};

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use stripstream_core::http::{build_http_client, USER_AGENT};
use utoipa::ToSchema;
use uuid::Uuid;

use super::torrent_client::{
    AddFailure, QbittorrentClient, TorrentClient, TorrentInfo, DOWNLOADS_SAVE_PATH,
};
use super::torrent_ledger::{self, Birth, NewTorrentDownload, TorrentBackend};
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

// ─── Config and login ───────────────────────────────────────────────────────

// Both live with the protocol adapter, in `torrent_client.rs`; the add path and
// the diagnostics probe keep reading them from here.
pub(crate) use super::torrent_client::{load_qbittorrent_config, qbittorrent_login};

// ─── Managed-download inference ───────────────────────────────────────────────

/// A download that will be tracked in `torrent_downloads` and auto-imported.
///
/// Both qBittorrent and AllDebrid accept the same "snatch" body; when the
/// caller does not supply a library/series, they are inferred so the download
/// still lands in the right place.
pub(crate) struct ManagedContext {
    pub library_id: Uuid,
    pub series_name: String,
}

/// Best-effort series name from a release URL: the `file=` query parameter
/// (Prowlarr proxy links), percent-decoded, run through the release parser.
pub(crate) fn infer_series_name_from_url(url: &str) -> Option<String> {
    let file = url.split("file=").nth(1)?;
    let encoded = file.split('&').next().unwrap_or(file);

    // Simple percent-decode + '+' to space.
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

    let name = parsers::extract_series_name_from_release(&result);
    (!name.is_empty()).then_some(name)
}

/// First library, used as the fallback when a snatch does not name one.
pub(crate) async fn first_library_id(pool: &sqlx::PgPool) -> Option<Uuid> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM libraries ORDER BY created_at LIMIT 1")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// Resolve the (library, series) a snatch belongs to, inferring what the caller
/// left out. `None` means the download is not managed (no tracking row).
pub(crate) async fn infer_managed_context(
    pool: &sqlx::PgPool,
    url: &str,
    library_id: Option<Uuid>,
    series_name: Option<String>,
) -> Option<ManagedContext> {
    let series_name = series_name
        .filter(|s| !s.is_empty())
        .or_else(|| infer_series_name_from_url(url))?;
    let library_id = match library_id {
        Some(id) => Some(id),
        None => first_library_id(pool).await,
    }?;

    Some(ManagedContext {
        library_id,
        series_name,
    })
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
    let managed = infer_managed_context(
        &state.pool,
        &body.url,
        body.library_id,
        body.series_name.clone(),
    )
    .await;
    let is_managed = managed.is_some();

    tracing::info!("[QBITTORRENT] Add torrent request: managed={is_managed}",);

    let (base_url, username, password) =
        load_qbittorrent_config(&state.pool)
            .await
            .inspect_err(|e| {
                tracing::error!("[QBITTORRENT] Failed to load config: {}", e.message);
            })?;

    let http = build_http_client(std::time::Duration::from_secs(30)).map_err(|e| {
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
            tracing::info!(
                "[QBITTORRENT] Resolved to .torrent file ({} bytes)",
                bytes.len()
            );
            resolved_magnet_url = None;
            torrent_bytes = Some(bytes);
        }
    }

    let sid = qbittorrent_login(&http, &base_url, &username, &password)
        .await
        .inspect_err(|e| {
            tracing::error!("[QBITTORRENT] Login failed: {}", e.message);
        })?;
    let client = QbittorrentClient::from_parts(base_url, http, sid);

    // Pre-generate the download ID; use a unique category per download so we can
    // reliably match the torrent back (tags/savepath are unreliable on qBittorrent 4.x).
    let download_id = if is_managed {
        Some(Uuid::new_v4())
    } else {
        None
    };
    let category = download_id.as_ref().map(|id| format!("sl-{id}"));

    // Create the category in qBittorrent before adding the torrent
    if let Some(ref cat) = category {
        client.create_category(cat).await;
    }

    // Only a managed torrent is filed under a category and told where to land.
    let savepath = is_managed.then_some(DOWNLOADS_SAVE_PATH);

    // The hash is known before the add for a magnet link (given directly, or
    // reached through an HTTP redirect). A `.torrent` file only gets its hash
    // afterwards, through our unique `sl-{id}` category — which by itself
    // proves Stripstream added it.
    let known_hash = extract_magnet_hash(&body.url)
        .or_else(|| resolved_magnet_url.as_deref().and_then(extract_magnet_hash));

    // When qBittorrent already has this torrent (added by the user, under
    // another category), adding it again is a no-op: the torrent is adopted,
    // never claimed, so Stripstream will not delete it or its files later.
    let mut adopted: Option<TorrentInfo> = None;
    if let (Some(managed), Some(hash), Some(cat)) =
        (managed.as_ref(), known_hash.as_deref(), category.as_deref())
    {
        let existing = match client.find_existing(hash).await {
            Ok(existing) => existing,
            Err(e) => {
                // Without an answer we cannot tell whether the torrent is the
                // user's: refuse rather than risk claiming (and later deleting) it.
                tracing::error!("[QBITTORRENT] Cannot check whether torrent {hash} exists: {e}");
                return Ok(Json(QBittorrentAddResponse {
                    success: false,
                    message: format!("qBittorrent torrent lookup failed: {e}"),
                    torrent_download_id: None,
                }));
            }
        };
        if let Some(existing) = existing.filter(|t| t.category.as_deref() != Some(cat)) {
            let content_path = existing.content_path.clone().unwrap_or_default();
            tracing::info!(
                "[QBITTORRENT] Torrent {} already in qBittorrent (category={:?}, content_path={})",
                existing.hash,
                existing.category,
                content_path
            );
            if existing.is_completed() {
                let content_exists =
                    !content_path.is_empty() && tokio::fs::metadata(&content_path).await.is_ok();
                if content_exists {
                    // Already downloaded: import right away, without adding.
                    let tid = download_id.unwrap();
                    let created = torrent_ledger::create(
                        &state.pool,
                        NewTorrentDownload {
                            qb_hash: Some(&existing.hash),
                            expected_volumes: body.expected_volumes.as_deref().unwrap_or(&[]),
                            replace_existing: body.replace_existing,
                            ..NewTorrentDownload::new(
                                tid,
                                managed.library_id,
                                &managed.series_name,
                                TorrentBackend::Qbittorrent,
                                Birth::CompletedAdopted {
                                    content_path: &content_path,
                                },
                            )
                        },
                    )
                    .await
                    .map_err(|e| {
                        tracing::error!(
                            "[QBITTORRENT] Failed to insert torrent_downloads row: {e}"
                        );
                        ApiError::from(e)
                    })?;
                    // Only an AllDebrid row has a uniqueness guard to lose.
                    if created.is_none() {
                        tracing::error!(
                            "[QBITTORRENT] torrent_downloads row {tid} was not inserted"
                        );
                        return Err(ApiError::internal("torrent download row was not inserted"));
                    }

                    tracing::info!("[QBITTORRENT] Duplicate torrent already completed, launching import immediately for {tid}");
                    super::torrent_import::spawn_import(
                        state.pool.clone(),
                        state.library_root.clone(),
                        tid,
                    );

                    return Ok(Json(QBittorrentAddResponse {
                        success: true,
                        message: "Duplicate torrent detected, import launched".to_string(),
                        torrent_download_id: Some(tid),
                    }));
                }

                // Stale residue: its content was already cleaned up. Drop the
                // torrent entry (never its files) and add the torrent anew.
                tracing::warn!(
                    "[QBITTORRENT] Duplicate torrent {} content no longer exists at {}, removing stale torrent",
                    existing.hash,
                    content_path
                );
                if let Err(e) = client.delete(&existing.hash, false).await {
                    tracing::warn!(
                        "[QBITTORRENT] Failed to remove stale torrent {}: {e}",
                        existing.hash
                    );
                }
            } else {
                adopted = Some(existing);
            }
        }
    }

    if adopted.is_none() {
        let added = if let Some(ref torrent_data) = torrent_bytes {
            client
                .add_file(torrent_data, savepath, category.as_deref())
                .await
        } else {
            // Pass magnet URL or original URL directly
            let url_to_send = resolved_magnet_url.as_deref().unwrap_or(&body.url);
            client
                .add_url(url_to_send, savepath, category.as_deref())
                .await
        };

        match added {
            Ok(()) => {}
            Err(AddFailure::Transport(e)) => {
                tracing::error!("[QBITTORRENT] Add torrent request failed: {e}");
                return Err(ApiError::internal(format!(
                    "qBittorrent add request failed: {e}"
                )));
            }
            Err(AddFailure::Refused { status, body }) => {
                tracing::error!("[QBITTORRENT] Add torrent failed — status={status}, body={body}");
                return Ok(Json(QBittorrentAddResponse {
                    success: false,
                    message: format!("qBittorrent returned {status}: {body}"),
                    torrent_download_id: None,
                }));
            }
        }
    }

    // If managed download: record in torrent_downloads
    let torrent_download_id = if let Some(managed) = managed.as_ref() {
        let library_id = managed.library_id;
        let series_name = managed.series_name.as_str();
        let expected_volumes = body.expected_volumes.as_deref().unwrap_or(&[]);

        // An adopted torrent belongs to the user: Stripstream must never
        // delete it afterwards (`qb_owned = FALSE`).
        let adopted_existing = adopted.is_some();
        let mut qb_hash = adopted.map(|t| t.hash).or(known_hash);
        if qb_hash.is_none() {
            if let Some(ref cat) = category {
                // .torrent files and magnets take time to be registered by
                // qBittorrent. Retry with increasing delays. Only a torrent
                // filed under our unique category (or saved in its directory)
                // matches, so a hash found here is always ours.
                for delay_secs in [2, 3, 5] {
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                    qb_hash = client.resolve_hash_by_category(cat).await;
                    if qb_hash.is_some() {
                        break;
                    }
                }

                if qb_hash.is_none() {
                    tracing::warn!("[QBITTORRENT] No hash resolved by category, searching all torrents for our category directory");
                    qb_hash = client.find_in_category_dir(cat).await;
                }
            }
        }

        let id = download_id.unwrap();
        // An adopted torrent belongs to the user: Stripstream must never
        // delete it afterwards (`qb_owned = FALSE`).
        let birth = if adopted_existing {
            Birth::Downloading
        } else {
            Birth::Queued
        };
        let created = torrent_ledger::create(
            &state.pool,
            NewTorrentDownload {
                qb_hash: qb_hash.as_deref(),
                expected_volumes,
                replace_existing: body.replace_existing,
                ..NewTorrentDownload::new(
                    id,
                    library_id,
                    series_name,
                    TorrentBackend::Qbittorrent,
                    birth,
                )
            },
        )
        .await
        .map_err(|e| {
            tracing::error!("[QBITTORRENT] Failed to insert torrent_downloads row: {e}");
            ApiError::from(e)
        })?;
        // Only an AllDebrid row has a uniqueness guard to lose.
        if created.is_none() {
            tracing::error!("[QBITTORRENT] torrent_downloads row {id} was not inserted");
            return Err(ApiError::internal("torrent download row was not inserted"));
        }

        tracing::info!(
            "[QBITTORRENT] Created torrent download {id} for {series_name}, qb_hash={qb_hash:?}, adopted={adopted_existing}"
        );

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

pub(crate) enum ResolvedTorrent {
    Magnet(String),
    TorrentFile(Vec<u8>),
}

/// Whether `ip` is an acceptable target for a user-supplied torrent URL.
///
/// Torrent links legitimately point at self-hosted indexers and Prowlarr
/// proxies on the LAN, so private ranges are allowed. Loopback, link-local
/// (including the `169.254.169.254` cloud metadata endpoint), unspecified,
/// broadcast and multicast addresses are rejected: they cannot host a real
/// torrent source and are classic SSRF targets.
fn is_allowed_torrent_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            !(v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                || octets[0] == 0)
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| !is_allowed_torrent_ip(IpAddr::V4(v4))))
        }
    }
}

/// Validate a torrent URL before fetching it.
///
/// Only HTTP(S) is accepted, and the host — literal or resolved through DNS —
/// must not be a loopback, link-local or metadata address. Redirect targets are
/// validated the same way by [`resolve_torrent_url`].
async fn ensure_safe_remote_url(url: &reqwest::Url) -> Result<(), String> {
    match url.scheme() {
        "http" | "https" => {}
        scheme => return Err(format!("unsupported URL scheme: {scheme}")),
    }

    let host = url
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;
    // `host_str` keeps the brackets around IPv6 literals.
    let check_host = host.trim_start_matches('[').trim_end_matches(']');
    let host_lower = check_host.to_ascii_lowercase();
    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".internal")
    {
        return Err(format!("refusing to fetch internal host: {host}"));
    }

    if let Ok(ip) = check_host.parse::<IpAddr>() {
        return if is_allowed_torrent_ip(ip) {
            Ok(())
        } else {
            Err(format!("refusing to fetch non-routable address: {host}"))
        };
    }

    let port = url.port_or_known_default().unwrap_or(443);
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((check_host, port))
        .await
        .map_err(|e| format!("cannot resolve {host}: {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("cannot resolve {host}"));
    }
    if addrs.iter().any(|addr| !is_allowed_torrent_ip(addr.ip())) {
        return Err(format!("refusing to fetch non-routable address for {host}"));
    }
    Ok(())
}

/// Resolve a torrent URL: if it's already a magnet link, return as-is.
/// Otherwise follow HTTP redirects. If the final URL is a magnet, return it.
/// If the response is a .torrent file, return the bytes.
///
/// Both the initial URL and every redirect target are checked by
/// [`ensure_safe_remote_url`] to prevent server-side request forgery.
pub(crate) async fn resolve_torrent_url(url: &str) -> Result<ResolvedTorrent, String> {
    // Already a magnet link — nothing to resolve
    if url.starts_with("magnet:") {
        return Ok(ResolvedTorrent::Magnet(url.to_string()));
    }

    // Build a client that does NOT follow redirects so we can inspect Location headers
    let no_redirect_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("failed to build redirect client: {e}"))?;

    let mut current_url =
        reqwest::Url::parse(url).map_err(|e| format!("invalid torrent URL: {e}"))?;
    ensure_safe_remote_url(&current_url).await?;

    for _ in 0..10 {
        let resp = no_redirect_client
            .get(current_url.clone())
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
                let next_url = current_url
                    .join(&location)
                    .map_err(|e| format!("invalid redirect URL: {e}"))?;
                ensure_safe_remote_url(&next_url).await?;
                tracing::debug!("[QBITTORRENT] Following redirect: {status} -> {next_url}");
                current_url = next_url;
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
pub(crate) fn extract_magnet_hash(url: &str) -> Option<String> {
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

    let client = build_http_client(std::time::Duration::from_secs(10))
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

#[cfg(test)]
#[path = "tests/qbittorrent.rs"]
mod tests;
