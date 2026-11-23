//! The qBittorrent protocol, behind one port.
//!
//! Two flows used to speak the qBittorrent Web API by hand: the poller in
//! `torrent_import.rs` (`torrents/info`, `torrents/delete`, category
//! resolution) and the add path in `qbittorrent.rs` (existence check, category
//! lookup, add). Each wrote its own request, its own cookie header and its own
//! decoding, so the same protocol had two spellings and the poll loop could only
//! be exercised against a real server.
//!
//! This module owns the wire format once:
//!
//! * [`TorrentClient`] is the port the flows consume — the torrents of a set of
//!   hashes or of a category, a category's hash, a delete and a category
//!   removal;
//! * [`QbittorrentClient`] is the adapter (base URL, HTTP client, SID cookie);
//! * [`TorrentInfo`] normalizes a `torrents/info` entry, and [`TorrentState`]
//!   turns its raw label into the vocabulary the poller reasons about;
//! * [`decide`] is the pure verdict of one poll for one tracked torrent, so the
//!   decision table is testable without a server, a database or a clock;
//! * [`FakeTorrentClient`] drives the poll loop in tests.
//!
//! The AllDebrid poller does **not** implement the port: its transfer is a
//! magnet whose status is a numeric `statusCode` reported against an
//! `external_id`, it has no category and no `qb_owned`, it deletes the magnet as
//! soon as the local copy is complete, and it carries fetch-attempt, backoff and
//! concurrency state that qBittorrent does not have. Forcing both into one
//! trait would only share the word "torrent" (ADR 0039).

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use stripstream_core::http::build_http_client;
use stripstream_core::settings::load_setting;

use crate::error::ApiError;

// ─── Configuration ──────────────────────────────────────────────────────────

/// Where qBittorrent lives and who logs in.
#[derive(Deserialize)]
struct QBittorrentConfig {
    url: String,
    username: String,
    password: String,
    /// Defaults to enabled so configurations saved before the flag existed keep working.
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

/// The configured qBittorrent base URL and credentials.
pub(crate) async fn load_qbittorrent_config(
    pool: &PgPool,
) -> Result<(String, String, String), ApiError> {
    let config = load_setting::<QBittorrentConfig>(pool, "qbittorrent")
        .await
        .map_err(|e| ApiError::internal(format!("invalid qbittorrent config: {e}")))?
        .ok_or_else(|| ApiError::bad_request("qBittorrent is not configured"))?;

    if !config.enabled {
        return Err(ApiError::bad_request("qBittorrent is disabled"));
    }

    if config.url.is_empty() || config.username.is_empty() {
        return Err(ApiError::bad_request(
            "qBittorrent URL and username must be configured in settings",
        ));
    }

    let url = config.url.trim_end_matches('/').to_string();
    Ok((url, config.username, config.password))
}

/// Log in and return the `SID` cookie qBittorrent issued.
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

/// Where qBittorrent stores the torrents Stripstream adds.
pub(super) const DOWNLOADS_SAVE_PATH: &str = "/downloads";

// ─── Normalized torrent ─────────────────────────────────────────────────────

/// Completed states in qBittorrent: torrent is fully downloaded (seeding,
/// queued for seeding, or stopped after completion — `pausedUP` before 5.0,
/// `stoppedUP` since).
pub(super) const QB_COMPLETED_STATES: &[&str] = &[
    "uploading",
    "stalledUP",
    "pausedUP",
    "stoppedUP",
    "queuedUP",
    "checkingUP",
    "forcedUP",
];

/// Download paused by the user (`pausedDL` before qBittorrent 5.0, `stoppedDL`
/// since). Never a failure: the row waits for the user to resume it.
const QB_PAUSED_STATES: &[&str] = &["pausedDL", "stoppedDL"];

/// No peer is sending data. Transient by nature: the torrent only fails once
/// it stayed there longer than the stall timeout.
const QB_STALLED_STATES: &[&str] = &["stalledDL"];

/// States qBittorrent cannot recover from without the user: the torrent fails
/// immediately.
const QB_FAILED_STATES: &[&str] = &["error", "missingFiles"];

/// What a qBittorrent state label means to the poller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TorrentState {
    /// Fully downloaded: hand over to the import.
    Completed,
    /// Paused by the user: keep the row, never delete anything.
    Paused,
    /// No peer: fail only once the stall lasted longer than the timeout.
    Stalled,
    /// Unrecoverable: fail now.
    Failed,
    /// Downloading, fetching metadata, checking, queued…: just track progress.
    InProgress,
}

impl TorrentState {
    /// Classify the raw label qBittorrent reported.
    pub(super) fn of(label: &str) -> Self {
        if QB_COMPLETED_STATES.contains(&label) {
            Self::Completed
        } else if QB_PAUSED_STATES.contains(&label) {
            Self::Paused
        } else if QB_STALLED_STATES.contains(&label) {
            Self::Stalled
        } else if QB_FAILED_STATES.contains(&label) {
            Self::Failed
        } else {
            Self::InProgress
        }
    }
}

/// One torrent as qBittorrent reports it, normalized.
#[derive(Debug, Clone)]
pub(super) struct TorrentInfo {
    pub hash: String,
    /// Raw qBittorrent label (`stalledUP`, `pausedDL`, …).
    pub state: String,
    pub content_path: Option<String>,
    pub save_path: Option<String>,
    pub name: Option<String>,
    pub category: Option<String>,
    pub progress: f64,
    pub total_size: i64,
    pub speed: i64,
    pub eta: i64,
}

impl TorrentInfo {
    pub(super) fn state_kind(&self) -> TorrentState {
        TorrentState::of(&self.state)
    }

    /// Whether the transfer is over, by fraction or by state.
    pub(super) fn is_completed(&self) -> bool {
        self.progress >= 1.0 || self.state_kind() == TorrentState::Completed
    }
}

/// The `torrents/info` wire format.
#[derive(Deserialize)]
struct QbTorrentInfo {
    hash: String,
    state: String,
    content_path: Option<String>,
    save_path: Option<String>,
    name: Option<String>,
    category: Option<String>,
    #[serde(default)]
    progress: f64,
    #[serde(default)]
    total_size: i64,
    #[serde(default)]
    dlspeed: i64,
    #[serde(default)]
    eta: i64,
}

impl From<QbTorrentInfo> for TorrentInfo {
    fn from(raw: QbTorrentInfo) -> Self {
        Self {
            hash: raw.hash,
            state: raw.state,
            content_path: raw.content_path,
            save_path: raw.save_path,
            name: raw.name,
            category: raw.category,
            progress: raw.progress,
            total_size: raw.total_size,
            speed: raw.dlspeed,
            eta: raw.eta,
        }
    }
}

// ─── The poller's decision ──────────────────────────────────────────────────

/// A `downloading` qBittorrent row the poller tracks.
#[derive(Debug, Clone)]
pub(super) struct TrackedTorrent {
    pub id: Uuid,
    pub hash: String,
    pub created_at: DateTime<Utc>,
    pub qb_owned: bool,
    /// When the row was first seen without any peer; `None` until armed.
    pub stall_started_at: Option<DateTime<Utc>>,
}

/// What the poller does with one tracked torrent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Decision {
    /// Downloading (or metadata/check/queue): persist progress, clear a stall.
    Track,
    /// Paused by the user: persist progress, never touch the torrent.
    LeaveAlone,
    /// No peer for now: arm the stall marker and wait.
    Stall,
    /// Give up on the torrent with this message.
    Fail(String),
    /// Fully downloaded: hand over to the import.
    Complete,
}

/// Whether a torrent stalled since `stall_started` has exhausted `timeout`.
pub(super) fn stall_expired(
    stall_started: DateTime<Utc>,
    now: DateTime<Utc>,
    timeout: Duration,
) -> bool {
    let timeout = chrono::Duration::from_std(timeout).unwrap_or(chrono::Duration::MAX);
    now - stall_started >= timeout
}

/// The verdict of one poll for one tracked torrent.
///
/// The only inputs are the torrent, the row and the clock, so the whole table is
/// a unit test: a completed torrent completes, a paused one is left alone, a
/// failed one fails now, a stalled one fails only once its marker is older than
/// the timeout, and anything else is just tracked.
pub(super) fn decide(
    info: &TorrentInfo,
    row: &TrackedTorrent,
    now: DateTime<Utc>,
    stall_timeout: Duration,
) -> Decision {
    match info.state_kind() {
        TorrentState::Completed => Decision::Complete,
        TorrentState::Paused => Decision::LeaveAlone,
        TorrentState::Failed => Decision::Fail(format!(
            "Torrent failed in qBittorrent (state: {})",
            info.state
        )),
        TorrentState::Stalled => match row.stall_started_at {
            Some(started) if stall_expired(started, now, stall_timeout) => Decision::Fail(format!(
                "Torrent stalled in qBittorrent for {} min (state: {})",
                (now - started).num_minutes(),
                info.state
            )),
            _ => Decision::Stall,
        },
        TorrentState::InProgress => Decision::Track,
    }
}

/// Tracked hashes that a successful `torrents/info` answer did not list.
/// qBittorrent reports lowercase hex; the comparison ignores case so a hash
/// stored from a magnet link in another case still matches.
pub(super) fn missing_hashes<'a>(tracked: &'a [String], reported: &[&str]) -> Vec<&'a str> {
    tracked
        .iter()
        .filter(|hash| !reported.iter().any(|r| r.eq_ignore_ascii_case(hash)))
        .map(String::as_str)
        .collect()
}

// ─── The port ───────────────────────────────────────────────────────────────

/// The qBittorrent operations the download flows need.
///
/// Kept to what the poller and the cleanup paths actually do: read torrents by
/// hash or category, resolve a category's hash, delete a torrent, remove a
/// category. Adding a torrent stays an inherent method of the adapter because
/// only the add handler uses it.
#[async_trait]
pub(super) trait TorrentClient: Send + Sync {
    /// The torrents among `hashes` qBittorrent still knows.
    ///
    /// Only a successful, fully decoded answer may prove a torrent is gone: an
    /// error must reach the caller so it leaves the rows alone.
    async fn torrents_by_hash(&self, hashes: &[String]) -> anyhow::Result<Vec<TorrentInfo>>;

    /// The torrents filed under `category`.
    async fn torrents_by_category(&self, category: &str) -> anyhow::Result<Vec<TorrentInfo>>;

    /// The single torrent filed under `category`, falling back to a torrent
    /// stored inside its `sl-{id}` directory.
    async fn resolve_hash_by_category(&self, category: &str) -> Option<String>;

    /// Remove a torrent, optionally with its files.
    async fn delete(&self, hash: &str, delete_files: bool) -> anyhow::Result<()>;

    /// Remove a category. Best effort: a failure only leaves a stale category.
    async fn remove_category(&self, category: &str);
}

// ─── qBittorrent adapter ────────────────────────────────────────────────────

/// A failed `/torrents/add`.
#[derive(Debug)]
pub(super) enum AddFailure {
    /// The request never reached qBittorrent.
    Transport(String),
    /// qBittorrent answered with a non-success status.
    Refused {
        status: reqwest::StatusCode,
        body: String,
    },
}

/// A logged-in qBittorrent session.
#[derive(Debug, Clone)]
pub(super) struct QbittorrentClient {
    base_url: String,
    http: reqwest::Client,
    sid: String,
}

impl QbittorrentClient {
    pub(super) fn from_parts(base_url: String, http: reqwest::Client, sid: String) -> Self {
        Self {
            base_url,
            http,
            sid,
        }
    }

    /// Load the configured qBittorrent and log in.
    pub(super) async fn connect(pool: &PgPool) -> anyhow::Result<Self> {
        let (base_url, username, password) = load_qbittorrent_config(pool)
            .await
            .map_err(|e| anyhow::anyhow!("qBittorrent config: {}", e.message))?;

        let http = build_http_client(Duration::from_secs(10))?;

        let sid = qbittorrent_login(&http, &base_url, &username, &password)
            .await
            .map_err(|e| anyhow::anyhow!("qBittorrent login: {}", e.message))?;

        Ok(Self::from_parts(base_url, http, sid))
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/api/v2/{path}", self.base_url)
    }

    fn cookie(&self) -> String {
        format!("SID={}", self.sid)
    }

    /// `torrents/info`, with an optional query.
    async fn info(&self, query: &[(&str, &str)]) -> anyhow::Result<Vec<TorrentInfo>> {
        let resp = self
            .http
            .get(self.endpoint("torrents/info"))
            .query(query)
            .header("Cookie", self.cookie())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "qBittorrent API returned {}",
                resp.status()
            ));
        }

        let infos: Vec<QbTorrentInfo> = resp.json().await?;
        Ok(infos.into_iter().map(TorrentInfo::from).collect())
    }

    /// Every torrent, for the save-path fallback. `None` when the answer is not
    /// usable: the fallback is best effort.
    async fn all_torrents(&self) -> Option<Vec<TorrentInfo>> {
        let resp = self
            .http
            .get(self.endpoint("torrents/info"))
            .header("Cookie", self.cookie())
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        resp.json::<Vec<QbTorrentInfo>>()
            .await
            .ok()
            .map(|infos| infos.into_iter().map(TorrentInfo::from).collect())
    }

    /// The torrent qBittorrent already has under `hash`, if any. An error means
    /// qBittorrent did not answer usably: the caller cannot conclude it is absent.
    pub(super) async fn find_existing(&self, hash: &str) -> Result<Option<TorrentInfo>, String> {
        let torrents = self
            .info(&[("hashes", hash)])
            .await
            .map_err(|e| e.to_string())?;
        Ok(torrents
            .into_iter()
            .find(|t| t.hash.eq_ignore_ascii_case(hash)))
    }

    /// Last-resort lookup of the torrent we just added: one filed under
    /// `category` or stored inside its `sl-{id}` directory.
    pub(super) async fn find_in_category_dir(&self, category: &str) -> Option<String> {
        let torrents = self.all_torrents().await?;
        let hash = torrents
            .into_iter()
            .find(|t| {
                t.category.as_deref() == Some(category)
                    || t.save_path.as_deref().is_some_and(|p| p.contains(category))
                    || t.content_path
                        .as_deref()
                        .is_some_and(|p| p.contains(category))
            })
            .map(|t| t.hash)?;
        tracing::info!("[QBITTORRENT] Found torrent in category directory: hash={hash}");
        Some(hash)
    }

    /// File a new category under the downloads directory.
    pub(super) async fn create_category(&self, category: &str) {
        let _ = self
            .http
            .post(self.endpoint("torrents/createCategory"))
            .header("Cookie", self.cookie())
            .form(&[("category", category), ("savePath", DOWNLOADS_SAVE_PATH)])
            .send()
            .await;
    }

    /// Add a torrent by URL (a magnet link or an indexer URL).
    pub(super) async fn add_url(
        &self,
        url: &str,
        savepath: Option<&str>,
        category: Option<&str>,
    ) -> Result<(), AddFailure> {
        let mut form: Vec<(&str, &str)> = vec![("urls", url)];
        if let Some(savepath) = savepath {
            form.push(("savepath", savepath));
        }
        if let Some(category) = category {
            form.push(("category", category));
        }

        let resp = self
            .http
            .post(self.endpoint("torrents/add"))
            .header("Cookie", self.cookie())
            .form(&form)
            .send()
            .await
            .map_err(|e| AddFailure::Transport(e.to_string()))?;

        refused_or_ok(resp).await
    }

    /// Add a torrent from `.torrent` bytes.
    pub(super) async fn add_file(
        &self,
        bytes: &[u8],
        savepath: Option<&str>,
        category: Option<&str>,
    ) -> Result<(), AddFailure> {
        let part = reqwest::multipart::Part::bytes(bytes.to_vec())
            .file_name("torrent.torrent")
            .mime_str("application/x-bittorrent")
            .map_err(|e| AddFailure::Transport(e.to_string()))?;
        let mut form = reqwest::multipart::Form::new().part("torrents", part);
        if let Some(savepath) = savepath {
            form = form.text("savepath", savepath.to_string());
        }
        if let Some(category) = category {
            form = form.text("category", category.to_string());
        }

        let resp = self
            .http
            .post(self.endpoint("torrents/add"))
            .header("Cookie", self.cookie())
            .multipart(form)
            .send()
            .await
            .map_err(|e| AddFailure::Transport(e.to_string()))?;

        refused_or_ok(resp).await
    }

    async fn set_category(&self, hash: &str, category: &str) {
        let _ = self
            .http
            .post(self.endpoint("torrents/setCategory"))
            .header("Cookie", self.cookie())
            .form(&[("hashes", hash), ("category", category)])
            .send()
            .await;
    }
}

async fn refused_or_ok(resp: reqwest::Response) -> Result<(), AddFailure> {
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Err(AddFailure::Refused { status, body })
}

#[async_trait]
impl TorrentClient for QbittorrentClient {
    async fn torrents_by_hash(&self, hashes: &[String]) -> anyhow::Result<Vec<TorrentInfo>> {
        let hashes = hashes.join("|");
        self.info(&[("hashes", &hashes)]).await
    }

    async fn torrents_by_category(&self, category: &str) -> anyhow::Result<Vec<TorrentInfo>> {
        self.info(&[("category", category)]).await
    }

    async fn resolve_hash_by_category(&self, category: &str) -> Option<String> {
        let torrents = match self.torrents_by_category(category).await {
            Ok(torrents) => torrents,
            Err(e) => {
                tracing::warn!(
                    "[QBITTORRENT] Failed to query torrents by category {category}: {e}"
                );
                return None;
            }
        };

        if torrents.len() == 1 {
            tracing::info!(
                "[QBITTORRENT] Resolved hash {} via category {category} ({:?})",
                torrents[0].hash,
                torrents[0].name
            );
            return Some(torrents[0].hash.clone());
        }

        if !torrents.is_empty() {
            tracing::warn!("[QBITTORRENT] Multiple torrents with category {category}, expected 1");
            return None;
        }

        // Fallback: search ALL torrents and match by save_path containing the download ID
        // This handles cases where qBittorrent didn't associate the category (magnet links)
        let download_id = category.strip_prefix("sl-").unwrap_or(category);
        let all = self.all_torrents().await?;
        let matched: Vec<&TorrentInfo> = all
            .iter()
            .filter(|t| {
                t.save_path
                    .as_deref()
                    .is_some_and(|p| p.contains(download_id))
                    || t.category
                        .as_deref()
                        .is_some_and(|c| c.contains(download_id))
            })
            .collect();

        if matched.len() == 1 {
            tracing::info!(
                "[QBITTORRENT] Resolved hash {} via save_path fallback for {category} ({:?})",
                matched[0].hash,
                matched[0].name
            );
            // Also fix the category on the torrent for future lookups
            self.set_category(&matched[0].hash, category).await;
            return Some(matched[0].hash.clone());
        }

        tracing::warn!(
            "[QBITTORRENT] No torrent found with category {category} (even with save_path fallback)"
        );
        None
    }

    async fn delete(&self, hash: &str, delete_files: bool) -> anyhow::Result<()> {
        let delete_files = if delete_files { "true" } else { "false" };
        let resp = self
            .http
            .post(self.endpoint("torrents/delete"))
            .header("Cookie", self.cookie())
            .form(&[("hashes", hash), ("deleteFiles", delete_files)])
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove torrent {hash}: {e}"))?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "qBittorrent refused to remove torrent {hash}: {}",
                resp.status()
            ));
        }

        Ok(())
    }

    async fn remove_category(&self, category: &str) {
        let _ = self
            .http
            .post(self.endpoint("torrents/removeCategories"))
            .header("Cookie", self.cookie())
            .form(&[("categories", category)])
            .send()
            .await;
    }
}

// ─── Test double ────────────────────────────────────────────────────────────

/// What one `delete` call looked like, for the delete-before-purge test.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(super) struct FakeDelete {
    pub hash: String,
    pub delete_files: bool,
    /// Whether the observed staging directory still existed at delete time.
    pub dir_existed: Option<bool>,
}

/// An in-memory [`TorrentClient`] for the poller and cleanup tests.
#[cfg(test)]
pub(super) struct FakeTorrentClient {
    infos: Vec<TorrentInfo>,
    resolution: HashMap<String, String>,
    refuse_delete: bool,
    fail_info: bool,
    observe_dir: Option<std::path::PathBuf>,
    deletes: Mutex<Vec<FakeDelete>>,
    removed_categories: Mutex<Vec<String>>,
}

#[cfg(test)]
impl FakeTorrentClient {
    pub(super) fn new(infos: Vec<TorrentInfo>) -> Self {
        Self {
            infos,
            resolution: HashMap::new(),
            refuse_delete: false,
            fail_info: false,
            observe_dir: None,
            deletes: Mutex::new(Vec::new()),
            removed_categories: Mutex::new(Vec::new()),
        }
    }

    /// Answer [`TorrentClient::resolve_hash_by_category`] for `category`.
    #[must_use]
    pub(super) fn with_resolution(mut self, category: &str, hash: &str) -> Self {
        self.resolution
            .insert(category.to_string(), hash.to_string());
        self
    }

    /// Make every `delete` fail, as a qBittorrent refusal would.
    #[must_use]
    pub(super) fn refusing_delete(mut self) -> Self {
        self.refuse_delete = true;
        self
    }

    /// Make every `torrents/info` answer fail.
    #[must_use]
    pub(super) fn failing_info(mut self) -> Self {
        self.fail_info = true;
        self
    }

    /// Record whether `dir` still exists when a delete arrives.
    #[must_use]
    pub(super) fn observing(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.observe_dir = Some(dir.into());
        self
    }

    pub(super) fn deleted_hashes(&self) -> Vec<String> {
        self.deletes
            .lock()
            .unwrap()
            .iter()
            .map(|delete| delete.hash.clone())
            .collect()
    }

    pub(super) fn deletes(&self) -> Vec<FakeDelete> {
        self.deletes.lock().unwrap().clone()
    }

    pub(super) fn removed_categories(&self) -> Vec<String> {
        self.removed_categories.lock().unwrap().clone()
    }
}

#[cfg(test)]
#[async_trait]
impl TorrentClient for FakeTorrentClient {
    async fn torrents_by_hash(&self, hashes: &[String]) -> anyhow::Result<Vec<TorrentInfo>> {
        if self.fail_info {
            return Err(anyhow::anyhow!("qBittorrent API returned 500"));
        }
        Ok(self
            .infos
            .iter()
            .filter(|info| {
                hashes
                    .iter()
                    .any(|hash| hash.eq_ignore_ascii_case(&info.hash))
            })
            .cloned()
            .collect())
    }

    async fn torrents_by_category(&self, category: &str) -> anyhow::Result<Vec<TorrentInfo>> {
        if self.fail_info {
            return Err(anyhow::anyhow!("qBittorrent API returned 500"));
        }
        Ok(self
            .infos
            .iter()
            .filter(|info| info.category.as_deref() == Some(category))
            .cloned()
            .collect())
    }

    async fn resolve_hash_by_category(&self, category: &str) -> Option<String> {
        self.resolution.get(category).cloned()
    }

    async fn delete(&self, hash: &str, delete_files: bool) -> anyhow::Result<()> {
        if self.refuse_delete {
            return Err(anyhow::anyhow!(
                "qBittorrent refused to remove torrent {hash}: 500 Internal Server Error"
            ));
        }
        self.deletes.lock().unwrap().push(FakeDelete {
            hash: hash.to_string(),
            delete_files,
            dir_existed: self.observe_dir.as_ref().map(|dir| dir.exists()),
        });
        Ok(())
    }

    async fn remove_category(&self, category: &str) {
        self.removed_categories
            .lock()
            .unwrap()
            .push(category.to_string());
    }
}

// ─── Test constructors ──────────────────────────────────────────────────────

#[cfg(test)]
impl Default for TorrentInfo {
    fn default() -> Self {
        Self {
            hash: String::new(),
            state: "downloading".to_string(),
            content_path: None,
            save_path: None,
            name: None,
            category: None,
            progress: 0.0,
            total_size: 0,
            speed: 0,
            eta: 0,
        }
    }
}

#[cfg(test)]
impl TorrentInfo {
    pub(super) fn test(hash: &str, state: &str) -> Self {
        Self {
            hash: hash.to_string(),
            state: state.to_string(),
            ..Self::default()
        }
    }

    #[must_use]
    pub(super) fn with_content_path(mut self, path: &str) -> Self {
        self.content_path = Some(path.to_string());
        self
    }

    #[must_use]
    pub(super) fn with_save_path(mut self, path: &str) -> Self {
        self.save_path = Some(path.to_string());
        self
    }

    #[must_use]
    pub(super) fn with_progress(mut self, progress: f64) -> Self {
        self.progress = progress;
        self
    }
}

#[cfg(test)]
#[path = "tests/torrent_client.rs"]
mod tests;
