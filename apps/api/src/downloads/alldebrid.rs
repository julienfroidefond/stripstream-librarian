//! AllDebrid as an alternative download client for Prowlarr snatches.
//!
//! A snatch used to always go to qBittorrent. AllDebrid is a debrid service:
//! it downloads the torrent on its servers and exposes the resulting files as
//! direct HTTP links, so no local BitTorrent client, VPN or seeding is needed.
//!
//! The `torrent_downloads` table stays the single ledger for both clients:
//! `client` tells the poller/import/webhook which backend a row belongs to and
//! `external_id` stores the AllDebrid magnet id. Once the magnet is ready, the
//! poller streams its book files into the usual staging directory
//! (`/downloads/sl-{id}`) and hands the row to the shared torrent import
//! pipeline, so volume matching, renaming and notifications stay identical to
//! the qBittorrent flow.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use sqlx::{PgPool, Row};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};
use uuid::Uuid;

use stripstream_core::http::build_http_client;
use stripstream_core::settings::load_setting;

use super::import_pipeline::remap_downloads_path;
use super::qbittorrent::{
    extract_magnet_hash, infer_managed_context, resolve_torrent_url, QBittorrentAddRequest,
    QBittorrentAddResponse, QBittorrentTestResponse, ResolvedTorrent,
};
use super::target::{sanitize_filename, DownloadProgress};
use super::torrent_import::{is_torrent_import_enabled, process_torrent_import};
use crate::error::ApiError;
use crate::responses::OkResponse;
use crate::state::AppState;

/// Book formats an imported torrent may contain.
const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "pdf", "epub"];

const ALLDEBRID_API_BASE: &str = "https://api.alldebrid.com";
const ALLDEBRID_TIMEOUT: Duration = Duration::from_secs(60);

/// Streaming an unlocked file link must not share the API client: its 60s
/// *total* timeout would abort any book slower than that. Downloads instead
/// allow an idle connection to stay open between chunks, which still catches a
/// transfer that stopped delivering data without capping the whole file.
const ALLDEBRID_DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(120);

/// How many local fetch failures are tolerated before a ready magnet is marked
/// as errored (transient network failures must not kill the row immediately).
const MAX_FETCH_ATTEMPTS: u32 = 3;

/// How long an in-flight magnet may go without receiving a single byte before
/// it is considered unreachable (dead torrent or blocked tracker).
const STALL_TIMEOUT_MINUTES: i64 = 30;

/// How many `torrent_downloads` rows are handled concurrently per poll cycle.
/// A row streaming its files keeps its slot, so this also bounds how many
/// AllDebrid transfers can run at once. Handling rows one by one used to freeze
/// the whole poller for the duration of a single transfer.
const POLL_CONCURRENCY: usize = 4;

/// Ceiling for the exponential backoff applied while AllDebrid status calls
/// keep failing (rate limit, outage): 5s, 10s, 20s, 40s, then 60s.
const MAX_API_BACKOFF: Duration = Duration::from_secs(60);

// ─── Config ─────────────────────────────────────────────────────────────────

#[derive(Clone, Deserialize)]
pub(crate) struct AlldebridConfig {
    #[serde(default)]
    pub(crate) api_key: String,
    #[serde(default)]
    pub(crate) enabled: bool,
}

/// Hand-written so an accidental `{:?}` can never print the API key.
impl std::fmt::Debug for AlldebridConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlldebridConfig")
            .field(
                "api_key",
                if self.api_key.is_empty() {
                    &"<unset>"
                } else {
                    &"<redacted>"
                },
            )
            .field("enabled", &self.enabled)
            .finish()
    }
}

/// Read the `alldebrid` setting; `Ok(None)` when the row is absent.
pub(crate) async fn load_alldebrid_config(
    pool: &PgPool,
) -> Result<Option<AlldebridConfig>, ApiError> {
    load_setting::<AlldebridConfig>(pool, "alldebrid")
        .await
        .map_err(|e| ApiError::internal(format!("invalid alldebrid config: {e}")))
}

async fn alldebrid_enabled(pool: &PgPool) -> bool {
    load_alldebrid_config(pool)
        .await
        .ok()
        .flatten()
        .map(|c| c.enabled)
        .unwrap_or(false)
}

/// Build a client from settings, requiring the integration to be enabled and
/// the API key to be present.
pub(crate) async fn load_alldebrid_client(pool: &PgPool) -> Result<AlldebridClient, ApiError> {
    let config = load_alldebrid_config(pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("AllDebrid is not configured"))?;

    if !config.enabled {
        return Err(ApiError::bad_request("AllDebrid is disabled"));
    }
    let api_key = config.api_key.trim().to_string();
    if api_key.is_empty() {
        return Err(ApiError::bad_request(
            "AllDebrid API key must be configured in settings",
        ));
    }

    AlldebridClient::new(api_key)
}

// ─── Client ─────────────────────────────────────────────────────────────────

pub(crate) struct AlldebridClient {
    http: reqwest::Client,
    /// Dedicated client for streaming unlocked file links; see
    /// [`ALLDEBRID_DOWNLOAD_READ_TIMEOUT`].
    download_http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl AlldebridClient {
    pub(crate) fn new(api_key: String) -> Result<Self, ApiError> {
        Self::with_base_url(api_key, ALLDEBRID_API_BASE)
    }

    /// Same as [`Self::new`] against a caller-supplied base URL (tests).
    fn with_base_url(api_key: String, base_url: &str) -> Result<Self, ApiError> {
        let http = build_http_client(ALLDEBRID_TIMEOUT)
            .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;
        let download_http = reqwest::Client::builder()
            .read_timeout(ALLDEBRID_DOWNLOAD_READ_TIMEOUT)
            .build()
            .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;
        Ok(Self {
            http,
            download_http,
            api_key,
            base_url: base_url.to_string(),
        })
    }

    /// POST a form-encoded request and return the raw response body.
    async fn post_form_raw(
        &self,
        path: &str,
        form: &[(String, String)],
    ) -> Result<String, ApiError> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .bearer_auth(&self.api_key)
            .form(form)
            .send()
            .await
            .map_err(|e| {
                ApiError::internal(format!(
                    "AllDebrid request failed: {}",
                    describe_http_failure(&e)
                ))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            ApiError::internal(format!(
                "AllDebrid response could not be read: {}",
                describe_http_failure(&e)
            ))
        })?;
        if !status.is_success() {
            return Err(ApiError::internal(format!(
                "AllDebrid returned HTTP {status}"
            )));
        }
        Ok(body)
    }

    fn parse_envelope<T>(body: &str) -> Result<Envelope<T>, ApiError>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_str(body)
            .map_err(|e| ApiError::internal(format!("AllDebrid returned malformed JSON: {e}")))
    }

    /// POST a form-encoded request and decode the `data` part of the envelope.
    async fn post_form<T>(&self, path: &str, form: &[(String, String)]) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
    {
        let body = self.post_form_raw(path, form).await?;
        Self::parse_envelope::<T>(&body)?.into_data()
    }

    /// POST a form-encoded request whose response carries no payload worth
    /// reading (delete, restart); only the envelope status matters.
    async fn post_form_unit(&self, path: &str, form: &[(String, String)]) -> Result<(), ApiError> {
        let body = self.post_form_raw(path, form).await?;
        Self::parse_envelope::<serde_json::Value>(&body)?
            .into_result()
            .map(|_| ())
    }

    pub(crate) async fn get_user(&self) -> Result<UserInfo, ApiError> {
        let data: UserData = self.post_form("/v4/user", &[]).await?;
        data.user
            .ok_or_else(|| ApiError::internal("AllDebrid returned no user"))
    }

    async fn upload_magnet(&self, magnet: &str) -> Result<MagnetUploadEntry, ApiError> {
        let data: MagnetUploadData = self
            .post_form(
                "/v4/magnet/upload",
                &[("magnets[]".to_string(), magnet.to_string())],
            )
            .await?;
        data.magnets
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::internal("AllDebrid returned no magnet"))
    }

    async fn upload_torrent_file(
        &self,
        filename: &str,
        bytes: Vec<u8>,
    ) -> Result<MagnetUploadEntry, ApiError> {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename.to_string())
            .mime_str("application/x-bittorrent")
            .map_err(|e| ApiError::internal(format!("invalid torrent upload part: {e}")))?;
        let form = reqwest::multipart::Form::new().part("files[]", part);

        let response = self
            .http
            .post(format!("{}/v4/magnet/upload/file", self.base_url))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                ApiError::internal(format!(
                    "AllDebrid upload failed: {}",
                    describe_http_failure(&e)
                ))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            ApiError::internal(format!(
                "AllDebrid response could not be read: {}",
                describe_http_failure(&e)
            ))
        })?;
        if !status.is_success() {
            return Err(ApiError::internal(format!(
                "AllDebrid returned HTTP {status}"
            )));
        }

        let envelope: Envelope<FileUploadData> = serde_json::from_str(&body)
            .map_err(|e| ApiError::internal(format!("AllDebrid returned malformed JSON: {e}")))?;
        let data = envelope.into_data()?;
        data.files
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::internal("AllDebrid returned no file"))
    }

    async fn magnet_status(&self, id: &str) -> Result<MagnetStatusEntry, ApiError> {
        let data: MagnetStatusData = self
            .post_form("/v4.1/magnet/status", &[("id".to_string(), id.to_string())])
            .await?;
        data.magnets
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::not_found("AllDebrid magnet not found"))
    }

    async fn magnet_files(&self, id: &str) -> Result<Vec<BookFile>, ApiError> {
        let data: MagnetFilesData = self
            .post_form("/v4/magnet/files", &[("id[]".to_string(), id.to_string())])
            .await?;
        let entry = data
            .magnets
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::not_found("AllDebrid magnet not found"))?;
        if let Some(err) = entry.error {
            return Err(ApiError::internal(format!(
                "AllDebrid error {}: {}",
                err.code, err.message
            )));
        }
        Ok(flatten_book_files(&entry.files))
    }

    async fn unlock_link(&self, link: &str) -> Result<UnlockData, ApiError> {
        self.post_form("/v4/link/unlock", &[("link".to_string(), link.to_string())])
            .await
    }

    async fn delayed_link(&self, id: &str) -> Result<UnlockData, ApiError> {
        self.post_form("/v4/link/delayed", &[("id".to_string(), id.to_string())])
            .await
    }

    pub(crate) async fn delete_magnet(&self, id: &str) -> Result<(), ApiError> {
        self.post_form_unit("/v4/magnet/delete", &[("id".to_string(), id.to_string())])
            .await
    }

    async fn restart_magnet(&self, id: &str) -> Result<(), ApiError> {
        self.post_form_unit("/v4/magnet/restart", &[("id".to_string(), id.to_string())])
            .await
    }
}

// ─── Envelope & payloads ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    status: String,
    /// Absent `data`/`error` decode to `None` (serde treats `Option` fields as optional).
    data: Option<T>,
    error: Option<ApiErrorBody>,
}

impl<T> Envelope<T> {
    fn into_result(self) -> Result<Option<T>, ApiError> {
        if self.status == "success" {
            return Ok(self.data);
        }
        match self.error {
            Some(err) => Err(ApiError::internal(format!(
                "AllDebrid error {}: {}",
                err.code, err.message
            ))),
            None => Err(ApiError::internal("AllDebrid request failed")),
        }
    }

    fn into_data(self) -> Result<T, ApiError> {
        self.into_result()?
            .ok_or_else(|| ApiError::internal("AllDebrid returned no data"))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    code: String,
    #[serde(default)]
    message: String,
}

/// AllDebrid returns ids as integers in some endpoints and as strings in
/// others; keep both usable without a typed column.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum IdValue {
    Int(i64),
    Str(String),
}

impl IdValue {
    fn as_string(&self) -> String {
        match self {
            IdValue::Int(n) => n.to_string(),
            IdValue::Str(s) => s.clone(),
        }
    }
}

/// Accept either a single object or an array of objects.
fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        One(T),
        Many(Vec<T>),
    }

    Ok(match OneOrMany::<T>::deserialize(deserializer)? {
        OneOrMany::One(value) => vec![value],
        OneOrMany::Many(values) => values,
    })
}

#[derive(Debug, Deserialize)]
struct UserData {
    #[serde(default)]
    user: Option<UserInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserInfo {
    #[serde(default)]
    pub(crate) username: String,
    #[serde(default)]
    pub(crate) is_premium: bool,
    #[serde(default)]
    pub(crate) is_subscribed: bool,
    #[serde(default)]
    pub(crate) is_trial: bool,
}

#[derive(Debug, Deserialize)]
struct MagnetUploadData {
    #[serde(default, deserialize_with = "one_or_many")]
    magnets: Vec<MagnetUploadEntry>,
}

#[derive(Debug, Deserialize)]
struct FileUploadData {
    #[serde(default, deserialize_with = "one_or_many")]
    files: Vec<MagnetUploadEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MagnetUploadEntry {
    #[serde(default)]
    id: Option<IdValue>,
    #[serde(default)]
    hash: Option<String>,
    /// Torrent name reported by AllDebrid.
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    error: Option<ApiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct MagnetStatusData {
    #[serde(default, deserialize_with = "one_or_many")]
    magnets: Vec<MagnetStatusEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MagnetStatusEntry {
    #[serde(default)]
    size: Option<i64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    status_code: Option<i32>,
    #[serde(default)]
    downloaded: Option<i64>,
    #[serde(default)]
    download_speed: Option<i64>,
    #[serde(default)]
    seeders: Option<i64>,
    /// Torrent name (single file: the file name; multi-file: the folder name).
    #[serde(default)]
    filename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MagnetFilesData {
    #[serde(default, deserialize_with = "one_or_many")]
    magnets: Vec<MagnetFilesEntry>,
}

#[derive(Debug, Deserialize)]
struct MagnetFilesEntry {
    #[serde(default)]
    files: Vec<FileNode>,
    #[serde(default)]
    error: Option<ApiErrorBody>,
}

/// A node in the AllDebrid file tree: a folder carries `e` (entries), a file
/// carries `l` (download link) and `s` (size).
#[derive(Debug, Deserialize)]
struct FileNode {
    #[serde(default)]
    n: String,
    #[serde(default)]
    s: Option<i64>,
    #[serde(default)]
    l: Option<String>,
    #[serde(default)]
    e: Vec<FileNode>,
}

#[derive(Debug, Deserialize)]
struct UnlockData {
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    delayed: Option<IdValue>,
}

/// A book file ready to be fetched from AllDebrid.
#[derive(Debug, Clone)]
struct BookFile {
    name: String,
    size: i64,
    link: String,
}

/// Recursively collect the book files of a magnet (ignoring cover images,
/// `.nfo`, subtitles and other non-book payloads).
fn flatten_book_files(nodes: &[FileNode]) -> Vec<BookFile> {
    fn visit(nodes: &[FileNode], out: &mut Vec<BookFile>) {
        for node in nodes {
            if !node.e.is_empty() {
                visit(&node.e, out);
                continue;
            }
            let Some(link) = node.l.as_ref().filter(|l| !l.is_empty()) else {
                continue;
            };
            if !is_book_file(&node.n) {
                continue;
            }
            out.push(BookFile {
                name: node.n.clone(),
                size: node.s.unwrap_or(0),
                link: link.clone(),
            });
        }
    }

    let mut files = Vec::new();
    visit(nodes, &mut files);
    files
}

fn is_book_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| BOOK_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

/// One-line description of a failed `reqwest` call that never repeats the
/// request URL. `reqwest::Error`'s own `Display` embeds it, and for AllDebrid
/// that URL can be a short-lived *signed* file link — which must not end up in
/// `torrent_downloads.error_message`, a field served by `GET /torrent-downloads`.
fn describe_http_failure(err: &reqwest::Error) -> String {
    let kind = if err.is_timeout() {
        "timeout"
    } else if err.is_connect() {
        "connection"
    } else if err.is_body() || err.is_decode() {
        "body"
    } else if err.is_redirect() {
        "redirect"
    } else {
        "request"
    };
    match err.status() {
        Some(status) => format!("{kind} error (HTTP {status})"),
        None => format!("{kind} error"),
    }
}

/// Human-readable text for an AllDebrid `statusCode`.
fn status_message(status_code: i32, status: Option<&str>) -> String {
    let text = match status_code {
        0 => "Magnet in queue",
        1 => "Downloading from peers",
        2 => "Compressing",
        3 => "Uploading to AllDebrid",
        4 => "Ready",
        5 => "Transfer failed",
        6 => "File too big",
        7 => "Internal error",
        8 => "Took too long to download",
        9 => "Missing file",
        10 => "Download took more than 72h",
        11 => "Deleted on the hoster website",
        12 | 13 => "Processing failed",
        14 => "Error while contacting tracker",
        15 => "File not available - no peer",
        _ => "Unknown status",
    };
    match status {
        Some(detail) if !detail.is_empty() && text == "Unknown status" => {
            format!("{text} ({detail})")
        }
        _ => text.to_string(),
    }
}

/// An in-flight magnet is stalled when it has not received a single byte and
/// has stayed that way for at least [`STALL_TIMEOUT_MINUTES`].
fn is_stalled(downloaded: i64, idle_minutes: i64) -> bool {
    downloaded == 0 && idle_minutes >= STALL_TIMEOUT_MINUTES
}

/// Message shown on the history row when a stalled magnet is abandoned.
/// `seeders` is `None` when AllDebrid did not report a count.
fn stall_message(idle_minutes: i64, seeders: Option<i64>) -> String {
    let seeders = match seeders {
        Some(n) => format!("{n} seeders"),
        None => "seeders unknown".to_string(),
    };
    format!(
        "No peer: 0 bytes downloaded in {} min ({seeders})",
        idle_minutes.max(1)
    )
}

/// AllDebrid refuses to "restart" a magnet it still considers active
/// (`MAGNET_PROCESSING`) — which is exactly the case for a stalled but still
/// alive magnet. Retry then means "keep watching it", not "surface an error".
fn is_already_processing(message: &str) -> bool {
    message.contains("MAGNET_PROCESSING")
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Add a torrent to AllDebrid (alternative to `POST /qbittorrent/add`).
#[utoipa::path(
    post,
    path = "/alldebrid/add",
    tag = "alldebrid",
    request_body = QBittorrentAddRequest,
    responses(
        (status = 200, body = QBittorrentAddResponse),
        (status = 400, description = "Bad request or AllDebrid not configured"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "AllDebrid connection error"),
    ),
    security(("Bearer" = []))
)]
pub async fn add_torrent(
    State(state): State<AppState>,
    Json(body): Json<QBittorrentAddRequest>,
) -> Result<Json<QBittorrentAddResponse>, ApiError> {
    if body.url.trim().is_empty() {
        return Err(ApiError::bad_request("url is required"));
    }

    let managed = infer_managed_context(
        &state.pool,
        &body.url,
        body.library_id,
        body.series_name.clone(),
    )
    .await;
    let is_managed = managed.is_some();

    info!("[ALLDEBRID] Add torrent request: managed={is_managed}");

    let client = load_alldebrid_client(&state.pool).await.inspect_err(|e| {
        tracing::error!("[ALLDEBRID] Failed to load config: {}", e.message);
    })?;

    let resolved = resolve_torrent_url(&body.url).await.map_err(|e| {
        tracing::error!("[ALLDEBRID] Failed to resolve torrent URL: {e}");
        ApiError::internal(format!("Failed to resolve torrent URL: {e}"))
    })?;

    let magnet_hash = match &resolved {
        ResolvedTorrent::Magnet(m) => extract_magnet_hash(m),
        ResolvedTorrent::TorrentFile(_) => extract_magnet_hash(&body.url),
    };

    // Cheap duplicate guard: a magnet link already being handled by AllDebrid
    // is reused instead of uploaded twice.
    if is_managed {
        if let Some(hash) = magnet_hash.as_deref() {
            if let Some(existing_id) = find_active_alldebrid_by_hash(&state.pool, hash).await {
                return Ok(Json(QBittorrentAddResponse {
                    success: true,
                    message: "This torrent is already being handled by AllDebrid".to_string(),
                    torrent_download_id: Some(existing_id),
                }));
            }
        }
    }

    let entry = match resolved {
        ResolvedTorrent::Magnet(m) => client.upload_magnet(&m).await?,
        ResolvedTorrent::TorrentFile(bytes) => {
            client.upload_torrent_file("torrent.torrent", bytes).await?
        }
    };

    if let Some(err) = &entry.error {
        warn!(
            "[ALLDEBRID] Magnet rejected ({}): {}",
            err.code, err.message
        );
        return Ok(Json(QBittorrentAddResponse {
            success: false,
            message: format!(
                "AllDebrid rejected the magnet ({}): {}",
                err.code, err.message
            ),
            torrent_download_id: None,
        }));
    }

    let external_id = entry
        .id
        .as_ref()
        .map(IdValue::as_string)
        .ok_or_else(|| ApiError::internal("AllDebrid did not return a magnet id"))?;
    let resolved_hash = entry.hash.clone().or(magnet_hash);

    let torrent_download_id = match managed.as_ref().filter(|_| is_managed) {
        Some(managed) => {
            // Secondary guard on the hash AllDebrid reports back (covers
            // .torrent uploads whose URL carried no hash).
            if let Some(hash) = resolved_hash.as_deref() {
                if let Some(existing_id) = find_active_alldebrid_by_hash(&state.pool, hash).await {
                    info!("[ALLDEBRID] Torrent {hash} already tracked by row {existing_id}");
                    // Free the freshly created (duplicate) magnet so it does not
                    // occupy one of AllDebrid's 30 active slots.
                    let _ = client.delete_magnet(&external_id).await;
                    return Ok(Json(QBittorrentAddResponse {
                        success: true,
                        message: "This torrent is already being handled by AllDebrid".to_string(),
                        torrent_download_id: Some(existing_id),
                    }));
                }
            }

            let id = Uuid::new_v4();
            let expected_volumes = body.expected_volumes.as_deref().unwrap_or(&[]);

            // The guard above cannot see a row that a concurrent request is
            // inserting right now; the unique partial index
            // `torrent_downloads_alldebrid_active_hash_key` is what actually
            // serialises two adds of the same magnet.
            let inserted = sqlx::query(
                "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, qb_hash, external_id, client, name, replace_existing) \
                 VALUES ($1, $2, $3, $4, $5, $6, 'alldebrid', $7, $8) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(managed.library_id)
            .bind(&managed.series_name)
            .bind(expected_volumes)
            .bind(resolved_hash.as_deref())
            .bind(&external_id)
            .bind(entry.name.as_deref())
            .bind(body.replace_existing)
            .execute(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("[ALLDEBRID] Failed to insert torrent_downloads row: {e}");
                ApiError::from(e)
            })?;

            if inserted.rows_affected() == 0 {
                // Lost the race: another request tracked this magnet first.
                // Drop the duplicate magnet and hand back the winning row.
                info!("[ALLDEBRID] Magnet {external_id} was tracked concurrently");
                let _ = client.delete_magnet(&external_id).await;
                let existing_id = match resolved_hash.as_deref() {
                    Some(hash) => find_active_alldebrid_by_hash(&state.pool, hash).await,
                    None => None,
                };
                return Ok(Json(QBittorrentAddResponse {
                    success: true,
                    message: "This torrent is already being handled by AllDebrid".to_string(),
                    torrent_download_id: existing_id,
                }));
            }

            info!(
                "[ALLDEBRID] Created torrent download {id} for {}, magnet {external_id}",
                managed.series_name
            );
            Some(id)
        }
        None => {
            info!("[ALLDEBRID] Magnet {external_id} added but not tracked (no library/series)");
            None
        }
    };

    Ok(Json(QBittorrentAddResponse {
        success: true,
        message: "Torrent added to AllDebrid".to_string(),
        torrent_download_id,
    }))
}

/// Test the AllDebrid connection and report the account state.
#[utoipa::path(
    get,
    path = "/alldebrid/test",
    tag = "alldebrid",
    responses(
        (status = 200, body = QBittorrentTestResponse),
        (status = 400, description = "AllDebrid not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn test_alldebrid(
    State(state): State<AppState>,
) -> Result<Json<QBittorrentTestResponse>, ApiError> {
    let client = load_alldebrid_client(&state.pool).await?;

    match client.get_user().await {
        Ok(user) => {
            if user.is_premium || user.is_subscribed || user.is_trial {
                Ok(Json(QBittorrentTestResponse {
                    success: true,
                    message: format!("Connected as {}", user.username),
                    version: Some(user.username),
                }))
            } else {
                Ok(Json(QBittorrentTestResponse {
                    success: false,
                    message: "AllDebrid account is not premium".to_string(),
                    version: Some(user.username),
                }))
            }
        }
        Err(e) => Ok(Json(QBittorrentTestResponse {
            success: false,
            message: e.message,
            version: None,
        })),
    }
}

/// Restart a failed AllDebrid magnet.
#[utoipa::path(
    post,
    path = "/alldebrid/downloads/{id}/restart",
    tag = "alldebrid",
    params(("id" = String, Path, description = "Torrent download UUID")),
    responses(
        (status = 200, description = "Magnet restart requested"),
        (status = 400, description = "Not in a restartable state or magnet id missing"),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn restart_torrent_download(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<OkResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT external_id, status FROM torrent_downloads WHERE id = $1 AND client = 'alldebrid'",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("AllDebrid torrent download not found"))?;

    let status: String = row.get("status");
    if status != "error" {
        return Err(ApiError::bad_request(
            "only a failed AllDebrid download can be restarted",
        ));
    }
    let external_id: Option<String> = row.get("external_id");
    let external_id =
        external_id.ok_or_else(|| ApiError::bad_request("AllDebrid magnet id is missing"))?;

    let client = load_alldebrid_client(&state.pool).await?;
    // A magnet AllDebrid still considers active (typically a stalled transfer —
    // it is still "processing", just without peers) cannot be restarted. In that
    // case Retry only re-arms the stall window so the poller keeps watching it.
    if let Err(e) = client.restart_magnet(&external_id).await {
        if !is_already_processing(&e.message) {
            return Err(e);
        }
    }

    sqlx::query(
        "UPDATE torrent_downloads SET status = 'downloading', error_message = NULL, \
         progress = 0, download_speed = 0, eta = 0, stall_started_at = NULL, updated_at = NOW() \
         WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    info!("[ALLDEBRID] Restarted magnet {external_id} for torrent {id}");
    Ok(Json(OkResponse::new()))
}

/// An active (not yet finished) AllDebrid row for a given info-hash.
async fn find_active_alldebrid_by_hash(pool: &PgPool, hash: &str) -> Option<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM torrent_downloads \
         WHERE client = 'alldebrid' AND LOWER(qb_hash) = LOWER($1) \
           AND status IN ('downloading', 'completed', 'importing') \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

// ─── Background poller ──────────────────────────────────────────────────────

/// What one poll cycle reported back, used to pick the next sleep.
struct PollOutcome {
    /// At least one AllDebrid row is still downloading.
    active: bool,
    /// Number of rows whose status call failed (rate limit, outage).
    status_errors: u32,
}

/// Poll AllDebrid magnets until their files are ready, then fetch and import.
pub async fn run_alldebrid_poller(pool: PgPool, interval_seconds: u64) {
    let idle_wait = Duration::from_secs(interval_seconds.max(5));
    let active_wait = Duration::from_secs(5);
    let error_wait = Duration::from_secs(15);
    let mut fetch_attempts: HashMap<Uuid, u32> = HashMap::new();
    // Grows while AllDebrid keeps rejecting status calls, reset on the first
    // clean cycle.
    let mut api_backoff = Duration::ZERO;

    loop {
        let wait = match poll_alldebrid_downloads(&pool, &mut fetch_attempts).await {
            Ok(outcome) if outcome.status_errors > 0 => {
                api_backoff = if api_backoff.is_zero() {
                    active_wait
                } else {
                    (api_backoff * 2).min(MAX_API_BACKOFF)
                };
                warn!(
                    "[ALLDEBRID] {} status call(s) failed, backing off {api_backoff:?}",
                    outcome.status_errors
                );
                api_backoff
            }
            Ok(outcome) => {
                api_backoff = Duration::ZERO;
                if outcome.active {
                    active_wait
                } else {
                    idle_wait
                }
            }
            Err(e) => {
                warn!("[ALLDEBRID] {:#}", e);
                let has_rows = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM torrent_downloads \
                     WHERE client = 'alldebrid' AND status = 'downloading'",
                )
                .fetch_one(&pool)
                .await
                .unwrap_or(0);
                if has_rows > 0 {
                    error_wait
                } else {
                    idle_wait
                }
            }
        };
        tokio::time::sleep(wait).await;
    }
}

/// Run one poll cycle. Rows are handled concurrently (bounded by
/// [`POLL_CONCURRENCY`]) because a row whose magnet is ready spends most of its
/// time streaming a file: handling rows one by one used to stall every other
/// magnet's progress for the duration of that transfer.
///
/// Note: AllDebrid exposes no way to batch `/magnet/status` by id (`id[]` is
/// ignored and returns the whole magnet list — hundreds of entries on a normal
/// account), so one status call per row remains the only option.
async fn poll_alldebrid_downloads(
    pool: &PgPool,
    fetch_attempts: &mut HashMap<Uuid, u32>,
) -> anyhow::Result<PollOutcome> {
    if !alldebrid_enabled(pool).await || !is_torrent_import_enabled(pool).await {
        return Ok(PollOutcome {
            active: false,
            status_errors: 0,
        });
    }

    let rows = sqlx::query(
        "SELECT id, external_id, created_at FROM torrent_downloads \
         WHERE client = 'alldebrid' AND status = 'downloading'",
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        fetch_attempts.clear();
        return Ok(PollOutcome {
            active: false,
            status_errors: 0,
        });
    }

    let client = Arc::new(
        load_alldebrid_client(pool)
            .await
            .map_err(|e| anyhow::anyhow!("AllDebrid config: {}", e.message))?,
    );
    // A row streaming its files holds its permit for the whole transfer, so this
    // also caps how many AllDebrid downloads run at once.
    let limit = Arc::new(tokio::sync::Semaphore::new(POLL_CONCURRENCY));
    let mut set: tokio::task::JoinSet<(Uuid, RowOutcome)> = tokio::task::JoinSet::new();

    for row in &rows {
        let tid = row.get::<Uuid, _>("id");
        let external_id = row.get::<Option<String>, _>("external_id");
        let created_at = row.get::<DateTime<Utc>, _>("created_at");
        let pool = pool.clone();
        let client = Arc::clone(&client);
        let limit = Arc::clone(&limit);
        set.spawn(async move {
            let Ok(_permit) = limit.acquire_owned().await else {
                // Only happens if the semaphore is closed, which nothing does.
                warn!("[ALLDEBRID] Download slot unavailable for {tid}");
                return (tid, RowOutcome::Skipped);
            };
            handle_row(pool, client, tid, external_id, created_at).await
        });
    }

    let mut outcomes = Vec::with_capacity(rows.len());
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(outcome) => outcomes.push(outcome),
            Err(e) => warn!("[ALLDEBRID] Poll task failed: {e}"),
        }
    }

    let mut active_ids = HashSet::new();
    let mut status_errors = 0u32;

    for (tid, outcome) in outcomes {
        active_ids.insert(tid);
        match outcome {
            RowOutcome::Skipped => {}
            RowOutcome::StatusError => status_errors += 1,
            RowOutcome::Fetched => {
                fetch_attempts.remove(&tid);
            }
            RowOutcome::FetchFailed(message) => {
                let attempts = fetch_attempts.entry(tid).or_insert(0);
                *attempts += 1;
                if *attempts >= MAX_FETCH_ATTEMPTS {
                    let message = format!("Failed to fetch files from AllDebrid: {message}");
                    warn!("[ALLDEBRID] {message} (torrent {tid})");
                    mark_error(pool, tid, &message).await;
                    fetch_attempts.remove(&tid);
                } else {
                    warn!(
                        "[ALLDEBRID] Fetch attempt {}/{} for torrent {tid} failed: {message}",
                        attempts, MAX_FETCH_ATTEMPTS
                    );
                }
            }
        }
    }

    fetch_attempts.retain(|id, _| active_ids.contains(id));

    let still_active = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM torrent_downloads \
         WHERE client = 'alldebrid' AND status = 'downloading'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    Ok(PollOutcome {
        active: still_active > 0,
        status_errors,
    })
}

/// What handling one row asked the next cycle to record.
enum RowOutcome {
    /// Still downloading, stalled, already errored, or without a magnet id.
    Skipped,
    /// The magnet status call itself failed.
    StatusError,
    /// The magnet was ready and its files were fetched locally.
    Fetched,
    /// The magnet was ready but the local fetch failed (retried up to
    /// [`MAX_FETCH_ATTEMPTS`]).
    FetchFailed(String),
}

/// Handle a single `downloading` row: fetch its magnet status and act on it.
///
/// Kept side-effect free regarding the shared fetch-attempt counter so rows can
/// be processed concurrently; the caller applies the returned outcome.
async fn handle_row(
    pool: PgPool,
    client: Arc<AlldebridClient>,
    tid: Uuid,
    external_id: Option<String>,
    created_at: DateTime<Utc>,
) -> (Uuid, RowOutcome) {
    let Some(external_id) = external_id else {
        let age = Utc::now() - created_at;
        if age.num_minutes() > 5 {
            warn!("[ALLDEBRID] Torrent {tid} has no magnet id after 5 minutes, marking as error");
            mark_error(&pool, tid, "AllDebrid magnet id was never recorded").await;
        }
        return (tid, RowOutcome::Skipped);
    };

    match client.magnet_status(&external_id).await {
        Ok(entry) => (
            tid,
            handle_magnet_status(&pool, client.as_ref(), tid, &external_id, &entry).await,
        ),
        Err(e) => {
            warn!(
                "[ALLDEBRID] Status request for {tid} (magnet {external_id}) failed: {}",
                e.message
            );
            (tid, RowOutcome::StatusError)
        }
    }
}

/// Live progress of an in-flight magnet, as reported by AllDebrid.
struct InflightProgress<'a> {
    downloaded: i64,
    progress: f32,
    speed: i64,
    eta: i64,
    /// AllDebrid omits the seeder count on some statuses; keep it optional so a
    /// stall message does not claim "0 seeders" when the value is unknown.
    seeders: Option<i64>,
    /// Torrent name from the magnet status, when AllDebrid reports one.
    name: Option<&'a str>,
}

/// Store the live progress of an in-flight magnet and return the instant it was
/// first seen at zero bytes (armed on first sight, kept across polls with
/// `COALESCE`, cleared as soon as a byte arrives). `None` means the transfer is
/// progressing, so there is nothing to time out.
async fn record_inflight_progress(
    pool: &PgPool,
    tid: Uuid,
    live: &InflightProgress<'_>,
) -> Option<DateTime<Utc>> {
    if live.downloaded > 0 {
        let result = sqlx::query(
            "UPDATE torrent_downloads SET progress = $1, download_speed = $2, eta = $3, \
             seeders = $4, name = COALESCE($6, name), stall_started_at = NULL, updated_at = NOW() \
             WHERE id = $5 AND status = 'downloading'",
        )
        .bind(live.progress)
        .bind(live.speed)
        .bind(live.eta)
        .bind(live.seeders.map(|s| s as i32))
        .bind(tid)
        .bind(live.name)
        .execute(pool)
        .await;
        if let Err(e) = result {
            warn!("[ALLDEBRID] Failed to store progress for torrent {tid}: {e}");
        }
        return None;
    }

    let stalled = sqlx::query_scalar::<_, DateTime<Utc>>(
        "UPDATE torrent_downloads SET progress = $1, download_speed = $2, eta = $3, \
         seeders = $4, name = COALESCE($6, name), \
         stall_started_at = COALESCE(stall_started_at, NOW()), updated_at = NOW() \
         WHERE id = $5 AND status = 'downloading' \
         RETURNING stall_started_at",
    )
    .bind(live.progress)
    .bind(live.speed)
    .bind(live.eta)
    .bind(live.seeders.map(|s| s as i32))
    .bind(tid)
    .bind(live.name)
    .fetch_optional(pool)
    .await;

    // `None` (no row matched, e.g. the row changed status since the query) is
    // normal; only a real query failure is worth logging.
    match stalled {
        Ok(stall) => stall,
        Err(e) => {
            warn!("[ALLDEBRID] Failed to arm the stall marker for torrent {tid}: {e}");
            None
        }
    }
}

async fn handle_magnet_status(
    pool: &PgPool,
    client: &AlldebridClient,
    tid: Uuid,
    external_id: &str,
    entry: &MagnetStatusEntry,
) -> RowOutcome {
    let status_code = entry.status_code.unwrap_or(0);

    if status_code >= 5 {
        let message = status_message(status_code, entry.status.as_deref());
        warn!("[ALLDEBRID] Magnet {external_id} (torrent {tid}) failed: {message}");
        mark_error(pool, tid, &message).await;
        let _ = client.delete_magnet(external_id).await;
        return RowOutcome::Skipped;
    }

    if status_code < 4 {
        let size = entry.size.unwrap_or(0);
        let downloaded = entry.downloaded.unwrap_or(0);
        let progress = if size > 0 {
            (downloaded as f64 / size as f64).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let speed = entry.download_speed.unwrap_or(0).max(0);
        let eta = if speed > 0 && size > downloaded {
            (size - downloaded) / speed
        } else {
            0
        };
        let seeders = entry.seeders;
        let name = entry.filename.as_deref().filter(|n| !n.is_empty());

        // A magnet that has not received a single byte is either queued or has
        // no peer. Remember when that started so a torrent the debrid service
        // can never fetch (0 seeder) does not sit "downloading" forever; the
        // marker is cleared as soon as a byte arrives and on a manual restart.
        let live = InflightProgress {
            downloaded,
            progress,
            speed,
            eta,
            seeders,
            name,
        };
        if let Some(stall_started) = record_inflight_progress(pool, tid, &live).await {
            let idle_minutes = (Utc::now() - stall_started).num_minutes();
            if is_stalled(downloaded, idle_minutes) {
                let message = stall_message(idle_minutes, seeders);
                warn!("[ALLDEBRID] Magnet {external_id} (torrent {tid}) stalled: {message}");
                mark_error(pool, tid, &message).await;
            }
        }
        return RowOutcome::Skipped;
    }

    // statusCode == 4: AllDebrid finished; fetch the files and hand over to
    // the shared torrent import pipeline.
    match download_ready_magnet(pool, client, tid, external_id).await {
        Ok(()) => RowOutcome::Fetched,
        Err(e) => RowOutcome::FetchFailed(format!("{e:#}")),
    }
}

/// Stream every book file of a ready magnet into the staging directory and
/// start the import.
async fn download_ready_magnet(
    pool: &PgPool,
    client: &AlldebridClient,
    tid: Uuid,
    external_id: &str,
) -> anyhow::Result<()> {
    let files = client
        .magnet_files(external_id)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    if files.is_empty() {
        let _ = client.delete_magnet(external_id).await;
        mark_error(pool, tid, "No book file (cbz/cbr/pdf/epub) in this torrent").await;
        return Ok(());
    }

    let staging_db = format!("/downloads/sl-{tid}");
    let staging_dir = remap_downloads_path(&staging_db);
    download_files_to(pool, client, tid, Path::new(&staging_dir), &files).await?;

    sqlx::query(
        "UPDATE torrent_downloads SET status = 'completed', content_path = $1, progress = 1, \
         download_speed = 0, eta = 0, updated_at = NOW() \
         WHERE id = $2 AND status = 'downloading'",
    )
    .bind(&staging_db)
    .bind(tid)
    .execute(pool)
    .await?;

    // The local copy is complete; drop the magnet from AllDebrid's 30-slot
    // active list so it can process other torrents.
    let _ = client.delete_magnet(external_id).await;

    info!("[ALLDEBRID] Torrent {tid} files fetched to {staging_db}, starting import");

    let pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = process_torrent_import(pool, tid).await {
            warn!("[ALLDEBRID] Import {tid} failed: {e:#}");
        }
    });

    Ok(())
}

/// Stream every book file into `staging_dir` (`.tmp` then rename) while keeping
/// the stored progress up to date. Split from [`download_ready_magnet`] so the
/// transfer itself can be exercised against a local HTTP server.
async fn download_files_to(
    pool: &PgPool,
    client: &AlldebridClient,
    tid: Uuid,
    staging_dir: &Path,
    files: &[BookFile],
) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(staging_dir)
        .await
        .with_context(|| format!("cannot create staging directory {}", staging_dir.display()))?;

    let total: i64 = files.iter().map(|f| f.size.max(0)).sum();
    let mut progress = DownloadProgress::new();
    let mut used_names: HashSet<String> = HashSet::new();

    for (index, file) in files.iter().enumerate() {
        let link = resolve_unlocked_link(client, &file.link).await?;
        let name = unique_name(&file.name, index, &mut used_names);
        let dest = staging_dir.join(&name);
        let tmp = PathBuf::from(format!("{}.tmp", dest.display()));

        info!(
            "[ALLDEBRID] Downloading '{}' for torrent {tid} ({} bytes)",
            name, file.size
        );

        // Use the download client: the API one caps the *whole* request at 60s,
        // which a book transfer routinely exceeds.
        let mut response = client.download_http.get(&link).send().await.map_err(|e| {
            anyhow::anyhow!("download request failed: {}", describe_http_failure(&e))
        })?;
        if !response.status().is_success() {
            anyhow::bail!("AllDebrid link returned HTTP {}", response.status());
        }

        let mut out = tokio::fs::File::create(&tmp)
            .await
            .with_context(|| format!("cannot create {}", tmp.display()))?;
        while let Some(chunk) = response.chunk().await.map_err(|e| {
            anyhow::anyhow!("reading response failed: {}", describe_http_failure(&e))
        })? {
            out.write_all(&chunk)
                .await
                .with_context(|| format!("cannot write to {}", tmp.display()))?;
            if let Some(written) = progress.advance(chunk.len() as i64) {
                report_progress(pool, tid, written, total).await;
            }
        }
        out.flush()
            .await
            .with_context(|| format!("cannot flush {}", tmp.display()))?;
        tokio::fs::rename(&tmp, &dest)
            .await
            .with_context(|| format!("cannot move {} into place", tmp.display()))?;
        report_progress(pool, tid, progress.written(), total).await;
    }

    Ok(())
}

/// Resolve a magnet file link into a direct download URL, waiting out
/// AllDebrid's "delayed" links when needed.
async fn resolve_unlocked_link(
    client: &AlldebridClient,
    file_link: &str,
) -> anyhow::Result<String> {
    let unlock = client
        .unlock_link(file_link)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    if let Some(link) = unlock.link.filter(|l| !l.is_empty()) {
        return Ok(link);
    }

    let Some(delayed) = unlock.delayed else {
        anyhow::bail!("AllDebrid unlock returned no link");
    };
    let delayed_id = delayed.as_string();

    for _ in 0..60 {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let data = client
            .delayed_link(&delayed_id)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e.message))?;
        if let Some(link) = data.link.filter(|l| !l.is_empty()) {
            return Ok(link);
        }
    }

    anyhow::bail!("AllDebrid delayed link {delayed_id} never became available")
}

/// Sanitize a magnet file name, disambiguating duplicates inside the staging
/// directory so two files from different folders cannot overwrite each other.
fn unique_name(original: &str, index: usize, used: &mut HashSet<String>) -> String {
    let sanitized = sanitize_filename(original);
    if used.insert(sanitized.clone()) {
        return sanitized;
    }

    let path = Path::new(&sanitized);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let fallback = if ext.is_empty() {
        format!("{stem}-{index}")
    } else {
        format!("{stem}-{index}.{ext}")
    };
    used.insert(fallback.clone());
    fallback
}

async fn report_progress(pool: &PgPool, tid: Uuid, written: i64, total: i64) {
    let fraction = if total > 0 {
        (written as f64 / total as f64).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    if let Err(e) = sqlx::query(
        "UPDATE torrent_downloads SET progress = $1, updated_at = NOW() \
         WHERE id = $2 AND status = 'downloading'",
    )
    .bind(fraction)
    .bind(tid)
    .execute(pool)
    .await
    {
        warn!("[ALLDEBRID] Failed to report progress for torrent {tid}: {e}");
    }
}

async fn mark_error(pool: &PgPool, tid: Uuid, message: &str) {
    let result = sqlx::query(
        "UPDATE torrent_downloads SET status = 'error', error_message = $1, \
         download_speed = 0, eta = 0, updated_at = NOW() \
         WHERE id = $2 AND status = 'downloading'",
    )
    .bind(message)
    .bind(tid)
    .execute(pool)
    .await;

    match result {
        Ok(done) if done.rows_affected() == 0 => {
            warn!("[ALLDEBRID] Torrent {tid} left '{message}' unrecorded: row is no longer downloading");
        }
        Ok(_) => {}
        Err(e) => warn!("[ALLDEBRID] Failed to mark torrent {tid} as error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn node(name: &str, link: Option<&str>, size: i64, children: Vec<FileNode>) -> FileNode {
        FileNode {
            n: name.to_string(),
            s: Some(size),
            l: link.map(str::to_string),
            e: children,
        }
    }

    #[test]
    fn is_book_file_accepts_book_extensions_case_insensitively() {
        assert!(is_book_file("Tome 1.CBZ"));
        assert!(is_book_file("tome.cbr"));
        assert!(is_book_file("scan.pdf"));
        assert!(is_book_file("novel.epub"));
        assert!(!is_book_file("cover.jpg"));
        assert!(!is_book_file("release.nfo"));
        assert!(!is_book_file("no-extension"));
    }

    #[test]
    fn flatten_book_files_recurses_folders_and_skips_non_books() {
        let tree = vec![
            node(
                "Series",
                None,
                0,
                vec![
                    node("Tome 1.cbz", Some("https://dl/1"), 100, vec![]),
                    node("cover.jpg", Some("https://dl/cover"), 10, vec![]),
                ],
            ),
            node(
                "Bonus",
                None,
                0,
                vec![node(
                    "Extras",
                    None,
                    0,
                    vec![node("Tome 2.pdf", Some("https://dl/2"), 200, vec![])],
                )],
            ),
            node("readme.txt", Some("https://dl/readme"), 5, vec![]),
            // A file node without a link cannot be downloaded and is dropped.
            node("Tome 3.epub", None, 300, vec![]),
        ];

        let files = flatten_book_files(&tree);
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["Tome 1.cbz", "Tome 2.pdf"]);
        assert_eq!(files[0].link, "https://dl/1");
        assert_eq!(files[1].size, 200);
    }

    #[test]
    fn envelope_success_decodes_data() {
        let envelope: Envelope<serde_json::Value> =
            serde_json::from_str(r#"{"status":"success","data":{"user":{"username":"bob"}}}"#)
                .unwrap();
        let data = envelope.into_data().unwrap();
        assert_eq!(data["user"]["username"], "bob");
    }

    #[test]
    fn envelope_error_surfaces_code_and_message() {
        let envelope: Envelope<serde_json::Value> = serde_json::from_str(
            r#"{"status":"error","error":{"code":"AUTH_BAD_APIKEY","message":"Invalid API key"}}"#,
        )
        .unwrap();
        let err = envelope.into_data().unwrap_err();
        assert!(err.message.contains("AUTH_BAD_APIKEY"));
        assert!(err.message.contains("Invalid API key"));
    }

    #[test]
    fn envelope_success_without_data_is_an_error() {
        let envelope: Envelope<serde_json::Value> =
            serde_json::from_str(r#"{"status":"success"}"#).unwrap();
        assert!(envelope.into_data().is_err());
    }

    #[test]
    fn unit_envelope_tolerates_missing_data() {
        let envelope: Envelope<serde_json::Value> =
            serde_json::from_str(r#"{"status":"success"}"#).unwrap();
        assert!(envelope.into_result().is_ok());
    }

    #[test]
    fn magnet_status_accepts_a_single_object_or_an_array() {
        let single: MagnetStatusData =
            serde_json::from_str(r#"{"magnets":{"id":42,"statusCode":1}}"#).unwrap();
        assert_eq!(single.magnets.len(), 1);
        assert_eq!(single.magnets[0].status_code, Some(1));

        let many: MagnetStatusData = serde_json::from_str(
            r#"{"magnets":[{"id":"7","statusCode":4},{"id":8,"statusCode":0}]}"#,
        )
        .unwrap();
        assert_eq!(many.magnets.len(), 2);
        assert_eq!(many.magnets[0].status_code, Some(4));
        assert_eq!(many.magnets[1].status_code, Some(0));
    }

    #[test]
    fn magnet_upload_id_accepts_an_int_or_a_string() {
        let int_id: MagnetUploadData =
            serde_json::from_str(r#"{"magnets":{"id":42,"hash":"abc"}}"#).unwrap();
        assert_eq!(int_id.magnets.len(), 1);
        assert_eq!(int_id.magnets[0].id.as_ref().unwrap().as_string(), "42");

        let str_id: MagnetUploadData =
            serde_json::from_str(r#"{"magnets":{"id":"42","hash":"abc"}}"#).unwrap();
        assert_eq!(str_id.magnets[0].id.as_ref().unwrap().as_string(), "42");
    }

    #[test]
    fn magnet_upload_entry_surfaces_per_entry_errors() {
        let data: MagnetUploadData = serde_json::from_str(
            r#"{"magnets":[{"error":{"code":"MAGNET_TOO_MANY_ACTIVE","message":"too many"}}]}"#,
        )
        .unwrap();
        assert_eq!(data.magnets.len(), 1);
        assert!(data.magnets[0].id.is_none());
        assert_eq!(
            data.magnets[0].error.as_ref().unwrap().code,
            "MAGNET_TOO_MANY_ACTIVE"
        );
    }

    #[test]
    fn status_message_marks_progress_and_error_codes() {
        assert_eq!(status_message(0, None), "Magnet in queue");
        assert_eq!(status_message(4, None), "Ready");
        assert_eq!(status_message(15, None), "File not available - no peer");
        assert_eq!(status_message(99, Some("weird")), "Unknown status (weird)");
    }

    #[test]
    fn stall_detection_needs_zero_bytes_and_time() {
        assert!(!is_stalled(0, STALL_TIMEOUT_MINUTES - 1));
        assert!(is_stalled(0, STALL_TIMEOUT_MINUTES));
        // A magnet that moved at least one byte is never considered stalled.
        assert!(!is_stalled(1, STALL_TIMEOUT_MINUTES * 10));

        assert_eq!(
            stall_message(42, Some(0)),
            "No peer: 0 bytes downloaded in 42 min (0 seeders)"
        );
        // Under a minute reads as "1 min" rather than "0 min".
        assert_eq!(
            stall_message(0, Some(3)),
            "No peer: 0 bytes downloaded in 1 min (3 seeders)"
        );
        // AllDebrid does not always report a seeder count; the message must not
        // pretend the transfer had zero seeders.
        assert_eq!(
            stall_message(10, None),
            "No peer: 0 bytes downloaded in 10 min (seeders unknown)"
        );
    }

    #[test]
    fn describe_http_failure_never_repeats_the_request_url() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // Nothing listens on port 1, so this always fails at connect time.
        let err = runtime
            .block_on(reqwest::get("http://127.0.0.1:1/unlocked?sig=supersecret"))
            .expect_err("connecting to port 1 must fail");

        let described = describe_http_failure(&err);
        assert!(
            !described.contains("127.0.0.1") && !described.contains("supersecret"),
            "the persisted error must not carry the signed link, got: {described}"
        );
        assert!(described.contains("error"), "got: {described}");
    }

    #[test]
    fn retry_keeps_watching_a_magnet_alldebrid_still_considers_active() {
        assert!(is_already_processing(
            "AllDebrid error MAGNET_PROCESSING: Magnet is processing or completed"
        ));
        assert!(!is_already_processing(
            "AllDebrid error MAGNET_INVALID_ID: Magnet not found"
        ));
    }

    #[test]
    fn unique_name_disambiguates_duplicates() {
        let mut used = HashSet::new();
        assert_eq!(unique_name("Tome 1.cbz", 0, &mut used), "Tome 1.cbz");
        assert_eq!(unique_name("Tome 1.cbz", 1, &mut used), "Tome 1-1.cbz");
        assert_eq!(unique_name("a/b.cbz", 2, &mut used), "a_b.cbz");
    }

    async fn seed_library(pool: &PgPool) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("test")
            .bind("/libraries/test")
            .execute(pool)
            .await
            .unwrap();
        library_id
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn active_alldebrid_lookup_is_case_insensitive(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, $2, 'Astérix', 'ABC123', '42', 'alldebrid', 'downloading')",
        )
        .bind(id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let found = find_active_alldebrid_by_hash(&pool, "abc123").await;
        assert_eq!(found, Some(id));

        // An unknown hash resolves to nothing.
        assert_eq!(find_active_alldebrid_by_hash(&pool, "deadbeef").await, None);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn active_lookup_ignores_finished_and_other_clients(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        let alldebrid_id = Uuid::new_v4();
        let qbittorrent_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, client, status) \
             VALUES ($1, $2, 'A', 'samehash', 'alldebrid', 'imported'), \
                    ($3, $2, 'B', 'samehash', 'qbittorrent', 'downloading')",
        )
        .bind(alldebrid_id)
        .bind(library_id)
        .bind(qbittorrent_id)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(find_active_alldebrid_by_hash(&pool, "samehash").await, None);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn client_column_defaults_to_qbittorrent(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name) VALUES ($1, $2, 'A')",
        )
        .bind(id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let client: String =
            sqlx::query_scalar("SELECT client FROM torrent_downloads WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(client, "qbittorrent");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn client_check_rejects_unknown_backend(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        let result = sqlx::query(
            "INSERT INTO torrent_downloads (library_id, series_name, client) \
             VALUES ($1, 'A', 'not-a-client')",
        )
        .bind(library_id)
        .execute(&pool)
        .await;

        assert!(
            result.is_err(),
            "the client CHECK constraint must reject unknown backends"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn inflight_progress_arms_then_clears_the_stall_marker(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, $2, 'Aposimz', 'HASH', '7', 'alldebrid', 'downloading')",
        )
        .bind(id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // The first zero-byte poll arms the marker, reports it and stores the
        // torrent name AllDebrid gave us...
        let stalled = InflightProgress {
            downloaded: 0,
            progress: 0.0,
            speed: 0,
            eta: 0,
            seeders: Some(0),
            name: Some("Aposimz.cbz"),
        };
        let armed = record_inflight_progress(&pool, id, &stalled)
            .await
            .expect("the first zero-byte poll arms the stall marker");
        // ...a second one keeps the original instant instead of pushing it back,
        // and a missing name must not overwrite the one already stored.
        let nameless = InflightProgress {
            name: None,
            ..stalled
        };
        let kept = record_inflight_progress(&pool, id, &nameless)
            .await
            .unwrap();
        assert_eq!(armed, kept);

        // As soon as a byte arrives the marker is cleared and seeders stored.
        let moving = InflightProgress {
            downloaded: 5,
            progress: 0.5,
            speed: 100,
            eta: 1,
            seeders: Some(12),
            name: None,
        };
        assert!(record_inflight_progress(&pool, id, &moving).await.is_none());

        let (stall, seeders, name): (Option<DateTime<Utc>>, Option<i32>, Option<String>) =
            sqlx::query_as(
                "SELECT stall_started_at, seeders, name FROM torrent_downloads WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(stall.is_none());
        assert_eq!(seeders, Some(12));
        assert_eq!(name.as_deref(), Some("Aposimz.cbz"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_second_active_alldebrid_row_for_the_same_hash_is_rejected(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, $2, 'Astérix', 'AABBCC', '1', 'alldebrid', 'downloading')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // Same hash, different casing: the unique partial index normalises case.
        let duplicate = sqlx::query(
            "INSERT INTO torrent_downloads (library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, 'Astérix', 'aabbcc', '2', 'alldebrid', 'downloading')",
        )
        .bind(library_id)
        .execute(&pool)
        .await;
        assert!(
            duplicate.is_err(),
            "two active AllDebrid rows for one hash must not be insertable"
        );

        // A finished row for the same hash stays possible (re-snatch after import)...
        let finished = sqlx::query(
            "INSERT INTO torrent_downloads (library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, 'Astérix', 'aabbcc', '3', 'alldebrid', 'imported')",
        )
        .bind(library_id)
        .execute(&pool)
        .await;
        assert!(finished.is_ok(), "a finished duplicate must remain allowed");

        // ...and so is the same hash tracked by the other client.
        let other_client = sqlx::query(
            "INSERT INTO torrent_downloads (library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, 'Astérix', 'aabbcc', '4', 'qbittorrent', 'downloading')",
        )
        .bind(library_id)
        .execute(&pool)
        .await;
        assert!(other_client.is_ok(), "the other client keeps its own rules");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn the_race_safe_insert_reports_a_conflict_instead_of_failing(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, $2, 'Astérix', 'AABBCC', '1', 'alldebrid', 'downloading')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let outcome = sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, $2, 'Astérix', 'aabbcc', '2', 'alldebrid', 'downloading') \
             ON CONFLICT DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            outcome.rows_affected(),
            0,
            "the request that loses the race must see zero inserted rows"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_zero_byte_magnet_past_the_timeout_is_marked_as_error(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status, stall_started_at) \
             VALUES ($1, $2, 'Aposimz', 'HASH', '7', 'alldebrid', 'downloading', NOW() - INTERVAL '40 minutes')",
        )
        .bind(id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // No network is involved for a statusCode < 4 entry.
        let client = AlldebridClient::new("dummy-key".to_string()).unwrap();
        let entry = MagnetStatusEntry {
            size: Some(73_396_635),
            status: Some("Downloading".to_string()),
            status_code: Some(1),
            downloaded: Some(0),
            download_speed: Some(0),
            seeders: Some(0),
            filename: Some("Aposimz T04.cbr".to_string()),
        };

        let outcome = handle_magnet_status(&pool, &client, id, "7", &entry).await;
        assert!(matches!(outcome, RowOutcome::Skipped));

        let (status, message): (String, Option<String>) =
            sqlx::query_as("SELECT status, error_message FROM torrent_downloads WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(status, "error");
        let message = message.expect("the stall must be explained on the row");
        assert!(
            message.starts_with("No peer: 0 bytes downloaded in"),
            "got: {message}"
        );
        assert!(message.ends_with("(0 seeders)"), "got: {message}");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn restart_refuses_a_download_that_is_still_running(pool: PgPool) {
        let library_id = seed_library(&pool).await;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, $2, 'A', 'H', '7', 'alldebrid', 'downloading')",
        )
        .bind(id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let state = crate::state::test_state(pool.clone());
        let err = match restart_torrent_download(State(state), AxumPath(id)).await {
            Ok(_) => panic!("restarting a running download must be refused"),
            Err(e) => e,
        };
        assert_eq!(err.status, axum::http::StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn restart_reports_an_unknown_alldebrid_row_as_not_found(pool: PgPool) {
        let state = crate::state::test_state(pool.clone());
        let err = match restart_torrent_download(State(state), AxumPath(Uuid::new_v4())).await {
            Ok(_) => panic!("an unknown row must not be restartable"),
            Err(e) => e,
        };
        assert_eq!(err.status, axum::http::StatusCode::NOT_FOUND);
    }

    async fn seed_downloading_row(pool: &PgPool) -> Uuid {
        let library_id = seed_library(pool).await;
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, qb_hash, external_id, client, status) \
             VALUES ($1, $2, 'Aposimz', 'H', '7', 'alldebrid', 'downloading')",
        )
        .bind(id)
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    /// Mount `/v4/link/unlock` so it hands out a link to `file_path` on the
    /// mock server itself, then serve `body` at that path.
    async fn mock_unlock_and_file(server: &MockServer, file_path: &str, body: &'static [u8]) {
        use wiremock::matchers::{method, path};

        Mock::given(method("POST"))
            .and(path("/v4/link/unlock"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"status":"success","data":{{"link":"{}{file_path}"}}}}"#,
                server.uri()
            )))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(file_path))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(server)
            .await;
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn download_files_to_streams_a_book_and_records_completion(pool: PgPool) {
        let id = seed_downloading_row(&pool).await;
        let server = MockServer::start().await;
        mock_unlock_and_file(&server, "/files/1", b"pages").await;

        let client = AlldebridClient::with_base_url("key".to_string(), &server.uri()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let files = vec![BookFile {
            name: "Aposimz T04.cbr".to_string(),
            size: 5,
            link: "magnet-internal-link".to_string(),
        }];

        download_files_to(&pool, &client, id, dir.path(), &files)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read(dir.path().join("Aposimz T04.cbr")).unwrap(),
            b"pages"
        );
        assert!(
            !dir.path().join("Aposimz T04.cbr.tmp").exists(),
            "the temporary file must be renamed away"
        );
        let progress: f32 =
            sqlx::query_scalar("SELECT progress FROM torrent_downloads WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(progress, 1.0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn download_files_to_reports_a_link_that_is_not_served(pool: PgPool) {
        use wiremock::matchers::{method, path};

        let id = seed_downloading_row(&pool).await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v4/link/unlock"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"status":"success","data":{{"link":"{}/files/gone"}}}}"#,
                server.uri()
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/gone"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = AlldebridClient::with_base_url("key".to_string(), &server.uri()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let files = vec![BookFile {
            name: "Aposimz T04.cbr".to_string(),
            size: 5,
            link: "magnet-internal-link".to_string(),
        }];

        let err = download_files_to(&pool, &client, id, dir.path(), &files)
            .await
            .expect_err("a 500 from the file link must fail the fetch");

        assert!(
            err.to_string().contains("HTTP 500"),
            "the error must name the HTTP status, got: {err}"
        );
        assert!(
            !err.to_string().contains(&server.uri()),
            "the error must not repeat the signed link, got: {err}"
        );
    }
}
