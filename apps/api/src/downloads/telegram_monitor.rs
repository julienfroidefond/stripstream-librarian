use axum::{extract::State, Json};
use axum::extract::Path as AxumPath;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use tracing::{error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use parsers::extract_volume;

use crate::{error::ApiError, state::AppState};
use stripstream_core::paths::remap_libraries_path;

use super::import_pipeline::find_existing_series_dir;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramMonitorStatus {
    pub configured: bool,
    pub authorized: bool,
    pub phone: Option<String>,
    pub api_id: Option<i64>,
    pub sync_interval_minutes: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveTelegramMonitorSettingsRequest {
    pub api_id: i64,
    pub api_hash: String,
    pub phone: String,
    pub sync_interval_minutes: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyCodeRequest {
    pub code: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramSourceDto {
    pub id: String,
    pub channel_username: String,
    pub channel_title: Option<String>,
    pub library_id: Option<String>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSourceRequest {
    pub channel_username: String,
    pub library_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramBookLinkDto {
    pub id: String,
    pub source_id: String,
    pub channel_username: String,
    pub message_id: i64,
    pub filename: String,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub message_text: Option<String>,
    pub status: String,
    pub library_id: Option<String>,
    pub book_id: Option<String>,
    pub error_message: Option<String>,
    pub series_name: Option<String>,
    pub volume_number: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramAvailableBookDto {
    pub id: String,
    pub channel_username: String,
    pub message_id: i64,
    pub filename: String,
    pub file_size: Option<i64>,
    pub volume_number: Option<i32>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramAvailableGroupDto {
    pub series_name: String,
    pub series_id: Option<String>,
    pub library_id: String,
    pub library_name: String,
    pub owned_volumes: Vec<i32>,
    pub series_missing_count: i32,
    pub books: Vec<TelegramAvailableBookDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramDownloadItemDto {
    pub id: String,
    pub series_name: Option<String>,
    pub series_id: Option<String>,
    pub library_id: Option<String>,
    pub library_name: Option<String>,
    pub channel_username: String,
    pub filename: String,
    pub file_size: Option<i64>,
    pub volume_number: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadBookRequest {
    pub library_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SyncResult {
    pub synced: usize,
    pub new_books: usize,
    pub series_searched: usize,
    /// Series that returned at least one Telegram result: (query_name, message_count, extracted_names)
    #[serde(skip)]
    pub series_results: Vec<(String, usize, Vec<String>)>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn load_tg_settings(pool: &sqlx::PgPool) -> Option<(i64, String, String, Option<Vec<u8>>)> {
    let row = sqlx::query(
        "SELECT value FROM app_settings WHERE key = 'telegram_monitor'",
    )
    .fetch_optional(pool)
    .await
    .ok()??;

    let v: Value = row.get("value");
    let api_id = v.get("api_id")?.as_i64()?;
    let api_hash = v.get("api_hash")?.as_str()?.to_string();
    let phone = v.get("phone")?.as_str()?.to_string();
    let session_bytes = v
        .get("session_data")
        .and_then(|s| s.as_str())
        .and_then(|s| B64.decode(s).ok());

    Some((api_id, api_hash, phone, session_bytes))
}

async fn load_tg_sync_interval(pool: &sqlx::PgPool) -> i32 {
    sqlx::query_scalar::<_, Option<i32>>(
        "SELECT (value->>'sync_interval_minutes')::int FROM app_settings WHERE key = 'telegram_monitor'"
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .flatten()
    .unwrap_or(60)
}

/// Extract series name from a book filename by stripping volume/tome markers.
/// "One Piece - Tome 47.cbz" → "One Piece"
/// "Toriko T12.cbz" → "Toriko"
pub(super) fn extract_series_name_from_filename(filename: &str) -> String {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    // Strip Telegram channel attribution "@channel" at end of stem (no spaces in handle)
    let stem = if let Some(at_pos) = stem.rfind('@') {
        let after = &stem[at_pos + 1..];
        if !after.is_empty() && !after.contains(' ') {
            stem[..at_pos].trim_end()
        } else {
            stem
        }
    } else {
        stem
    };

    // Normalize underscores → spaces so both naming styles are handled uniformly
    let normalized = stem.replace('_', " ");
    let lower = normalized.to_lowercase();

    // Named patterns (longest first to avoid partial matches)
    let patterns: &[&str] = &[
        " - intégrale", " - integrale", " - hors-série", " - hors série", " - hors-serie",
        " - tome ", " - volume ", " - vol. ", " - vol ", " - chapter ", " - chapitre ",
        " - chap. ", " - chap ", " - ch. ",
        " - t.", " - t ",
        " intégrale", " integrale", " hors-série", " hors série",
        " tome ", " volume ", " vol. ", " vol ", " chapitre ",
    ];

    let mut earliest = normalized.len();
    for pattern in patterns {
        if let Some(pos) = lower.find(pattern) {
            if pos < earliest && pos > 0 {
                earliest = pos;
            }
        }
    }

    // " - \d" pattern: bare number between dashes e.g. "Series - 02 - Title" or "Series - 02"
    {
        let b = lower.as_bytes();
        let mut i = 3usize;
        while i < b.len() {
            if b[i - 3] == b' ' && b[i - 2] == b'-' && b[i - 1] == b' ' && b[i].is_ascii_digit() {
                let sep = i - 3;
                if sep > 0 && sep < earliest {
                    earliest = sep;
                }
                break;
            }
            i += 1;
        }
    }

    // " T\d", " V\d", " Ch\d", " #\d", " #Ch\d" and " - T\d", " - V\d", " - Ch\d", " - #Ch\d" patterns
    let bytes = lower.as_bytes();
    let mut i = 1usize;
    while i + 1 < bytes.len() {
        let prev = bytes[i - 1];
        let cur = bytes[i];
        if prev == b' ' {
            if cur == b't' || cur == b'v' {
                // " T\d" / " V\d"
                let j = i + 1;
                let k = if j < bytes.len() && (bytes[j] == b'.' || bytes[j] == b' ') { j + 1 } else { j };
                if k < bytes.len() && bytes[k].is_ascii_digit() && (i - 1) < earliest {
                    earliest = i - 1;
                }
            } else if cur == b'c' {
                // " Ch\d" — chapter marker like "Ch09"
                let j = i + 1;
                if j + 1 < bytes.len() && bytes[j] == b'h' && bytes[j + 1].is_ascii_digit() && (i - 1) < earliest {
                    earliest = i - 1;
                }
            } else if cur == b'#' {
                let j = i + 1;
                if j < bytes.len() && bytes[j].is_ascii_digit() && (i - 1) < earliest {
                    // " #\d"
                    earliest = i - 1;
                } else if j + 2 < bytes.len() && bytes[j] == b'c' && bytes[j + 1] == b'h' && bytes[j + 2].is_ascii_digit() && (i - 1) < earliest {
                    // " #Ch\d"
                    earliest = i - 1;
                }
            }
        }
        // " - T\d" / " - V\d" / " - Ch\d"
        if i >= 3 && bytes[i-3] == b' ' && bytes[i-2] == b'-' && bytes[i-1] == b' '
            && (cur == b't' || cur == b'v' || cur == b'c')
        {
            let sep = i - 3;
            if cur == b'c' {
                // " - Ch\d"
                let j = i + 1;
                if j + 1 < bytes.len() && bytes[j] == b'h' && bytes[j + 1].is_ascii_digit() && sep < earliest {
                    earliest = sep;
                }
            } else {
                let j = i + 1;
                let k = if j < bytes.len() && (bytes[j] == b'.' || bytes[j] == b' ') { j + 1 } else { j };
                if k < bytes.len() && bytes[k].is_ascii_digit() && sep < earliest {
                    earliest = sep;
                }
            }
        }
        // " - #\d" / " - #Ch\d" (hash after space-dash-space)
        if i >= 3 && bytes[i-3] == b' ' && bytes[i-2] == b'-' && bytes[i-1] == b' ' && cur == b'#' {
            let j = i + 1;
            let sep = i - 3;
            if j < bytes.len() && bytes[j].is_ascii_digit() && sep < earliest {
                // " - #\d"
                earliest = sep;
            } else if j + 2 < bytes.len() && bytes[j] == b'c' && bytes[j + 1] == b'h' && bytes[j + 2].is_ascii_digit() && sep < earliest {
                // " - #Ch\d"
                earliest = sep;
            }
        }
        // " -T\d" / " -V\d" (no space after dash, e.g. "Series -T01(...")
        if i >= 2 && bytes[i-2] == b' ' && bytes[i-1] == b'-'
            && (cur == b't' || cur == b'v')
        {
            let j = i + 1;
            if j < bytes.len() && bytes[j].is_ascii_digit() {
                let sep = i - 2;
                if sep < earliest { earliest = sep; }
            }
        }
        i += 1;
    }

    // Phases 4 & 5: bare number handling — only when no named pattern matched yet
    if earliest == normalized.len() {
        let b = lower.as_bytes();
        let n = b.len();

        // Phase 4: trailing " \d{1,3}$" (not 4-digit years)
        // If applied, skip phase 5 to preserve embedded title numbers.
        // e.g. "Roi Démon ... 10 Enfants Le 1" → strip " 1", not " 10"
        let trailing_applied = {
            let mut end = n;
            while end > 0 && b[end - 1].is_ascii_digit() { end -= 1; }
            let digit_count = n - end;
            if digit_count >= 1 && digit_count <= 3 && end > 0 && b[end - 1] == b' ' {
                let sep = end - 1;
                if sep >= 3 { earliest = sep; true } else { false }
            } else { false }
        };

        // Phase 5: middle " \d{1,3} " — only if no trailing number found.
        // e.g. "Dragon Ball Super 1 Les Guerriers" → "Dragon Ball Super"
        if !trailing_applied {
            let mut i = 1usize;
            while i < b.len() {
                if b[i - 1] == b' ' && b[i].is_ascii_digit() {
                    let mut j = i;
                    while j < b.len() && b[j].is_ascii_digit() { j += 1; }
                    let digit_count = j - i;
                    let after_ok = j == b.len() || b[j] == b' ' || b[j] == b'-';
                    let sep = i - 1;
                    if digit_count <= 3 && after_ok && sep >= 3 {
                        earliest = sep;
                        break;
                    }
                }
                i += 1;
            }
        }
    }

    normalized[..earliest].trim_end_matches([' ', '-', '_', '.']).to_string()
}

/// Find the physical target directory for a series in a library,
/// matching existing book files first, then existing directories, then fallback to new.
async fn resolve_target_dir(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
) -> anyhow::Result<String> {
    // 1. Try DB: find existing book files for this series
    let any_row = sqlx::query(
        "SELECT bf.abs_path FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND LOWER(unaccent(s.name)) = LOWER(unaccent($2)) \
         LIMIT 1",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    if let Some(r) = any_row {
        let abs_path: String = r.get("abs_path");
        let physical = remap_libraries_path(&abs_path);
        let parent = std::path::Path::new(&physical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(physical);
        info!("[TG] Series '{}' DB match → {}", series_name, parent);
        return Ok(parent);
    }

    // 2. No DB match: look for existing directory in library root
    let lib_row = sqlx::query("SELECT root_path FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(pool)
        .await?;
    let root_path: String = lib_row.get("root_path");
    let physical_root = remap_libraries_path(&root_path);

    let dir = find_existing_series_dir(&physical_root, series_name)
        .unwrap_or_else(|| format!("{}/{}", physical_root.trim_end_matches('/'), series_name));

    info!("[TG] Series '{}' dir → {}", series_name, dir);
    Ok(dir)
}

async fn save_session_to_db(pool: &sqlx::PgPool, session_bytes: Vec<u8>) -> Result<(), ApiError> {
    let b64 = B64.encode(&session_bytes);
    sqlx::query(
        "UPDATE app_settings \
         SET value = jsonb_set(value, '{session_data}', to_jsonb($1::text)), updated_at = NOW() \
         WHERE key = 'telegram_monitor'",
    )
    .bind(b64)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/status
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/status",
    tag = "telegram-monitor",
    responses((status = 200, body = TelegramMonitorStatus), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn get_status(
    State(state): State<AppState>,
) -> Result<Json<TelegramMonitorStatus>, ApiError> {
    let settings = load_tg_settings(&state.pool).await;
    let sync_interval_minutes = load_tg_sync_interval(&state.pool).await;

    let (configured, authorized, phone, api_id) = match settings {
        None => (false, false, None, None),
        Some((api_id, _api_hash, phone, session)) => {
            (true, session.is_some(), Some(phone), Some(api_id))
        }
    };

    Ok(Json(TelegramMonitorStatus { configured, authorized, phone, api_id, sync_interval_minutes }))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/settings
// ---------------------------------------------------------------------------

#[utoipa::path(
    post, path = "/telegram-monitor/settings",
    tag = "telegram-monitor",
    request_body = SaveTelegramMonitorSettingsRequest,
    responses((status = 200), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn save_settings(
    State(state): State<AppState>,
    Json(body): Json<SaveTelegramMonitorSettingsRequest>,
) -> Result<Json<Value>, ApiError> {
    let phone = body.phone.trim().to_string();
    let api_hash = body.api_hash.trim().to_string();

    // Preserve existing session (as base64) if api_id/hash unchanged
    let existing = load_tg_settings(&state.pool).await;
    let session_b64 = existing
        .filter(|(id, hash, _, _)| *id == body.api_id && hash == &api_hash)
        .and_then(|(_, _, _, bytes)| bytes)
        .map(|b| B64.encode(b));

    let sync_interval = body.sync_interval_minutes.unwrap_or(60).max(0);
    let value = serde_json::json!({
        "api_id": body.api_id,
        "api_hash": api_hash,
        "phone": phone,
        "session_data": session_b64,
        "sync_interval_minutes": sync_interval,
    });

    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('telegram_monitor', $1) \
         ON CONFLICT (key) DO UPDATE SET value = $1, updated_at = NOW()",
    )
    .bind(&value)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/auth/start  — sends SMS code via grammers
// ---------------------------------------------------------------------------

#[utoipa::path(
    post, path = "/telegram-monitor/auth/start",
    tag = "telegram-monitor",
    responses(
        (status = 200, description = "Code sent to phone"),
        (status = 400, description = "Not configured"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn start_auth(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let (api_id, api_hash, phone, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Telegram monitor not configured"))?;

    do_start_auth(
        state.pending_tg_auth.clone(),
        api_id as i32,
        api_hash,
        phone,
        session_bytes,
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true, "message": "Code sent to your phone/Telegram app" })))
}

async fn do_start_auth(
    handle: std::sync::Arc<tokio::sync::Mutex<Option<PendingAuth>>>,
    api_id: i32,
    api_hash: String,
    phone: String,
    session_bytes: Option<Vec<u8>>,
) -> Result<(), ApiError> {
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    let session = match session_bytes {
        Some(bytes) => Session::load(&bytes)
            .map_err(|e| ApiError::internal(format!("session load: {e}")))?,
        None => Session::new(),
    };

    let client = Client::connect(Config {
        session,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await
    .map_err(|e| ApiError::internal(format!("Telegram connect failed: {e}")))?;

    let token = client
        .request_login_code(&phone)
        .await
        .map_err(|e| ApiError::internal(format!("request_login_code failed: {e}")))?;

    let mut guard = handle.lock().await;
    *guard = Some(PendingAuth { client, token });

    Ok(())
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/auth/verify  — verifies code and saves session
// ---------------------------------------------------------------------------

#[utoipa::path(
    post, path = "/telegram-monitor/auth/verify",
    tag = "telegram-monitor",
    request_body = VerifyCodeRequest,
    responses(
        (status = 200, description = "Authenticated"),
        (status = 400, description = "Invalid code or not started"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn verify_auth(
    State(state): State<AppState>,
    Json(body): Json<VerifyCodeRequest>,
) -> Result<Json<Value>, ApiError> {
    do_verify_auth(
        state.pending_tg_auth.clone(),
        state.pool.clone(),
        body.code.trim().to_string(),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn do_verify_auth(
    handle: std::sync::Arc<tokio::sync::Mutex<Option<PendingAuth>>>,
    pool: sqlx::PgPool,
    code: String,
) -> Result<(), ApiError> {
    let mut guard = handle.lock().await;
    let pending = guard
        .take()
        .ok_or_else(|| ApiError::bad_request("No pending auth — call /auth/start first"))?;

    let PendingAuth { client, token } = pending;

    client
        .sign_in(&token, &code)
        .await
        .map_err(|e| ApiError::bad_request(format!("Sign in failed: {e}")))?;

    let session_bytes = client.session().save();
    save_session_to_db(&pool, session_bytes).await?;

    info!("Telegram session saved to DB");
    Ok(())
}

// ---------------------------------------------------------------------------
// DELETE /telegram-monitor/auth  — disconnect
// ---------------------------------------------------------------------------

#[utoipa::path(
    delete, path = "/telegram-monitor/auth",
    tag = "telegram-monitor",
    responses((status = 200), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn disconnect(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    // Clear session_data in DB
    sqlx::query(
        "UPDATE app_settings \
         SET value = value - 'session_data', updated_at = NOW() \
         WHERE key = 'telegram_monitor'",
    )
    .execute(&state.pool)
    .await?;

    let mut guard = state.pending_tg_auth.lock().await;
    *guard = None;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Manual job trigger: enqueue a pending telegram_sync job
// ---------------------------------------------------------------------------

pub async fn start_incremental_sync_job(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'telegram_monitor' AND value->>'session_data' IS NOT NULL)",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !session_exists {
        return Err(ApiError::bad_request("Telegram not authorized"));
    }

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id IS NULL AND type = 'telegram_sync_incremental' AND status IN ('pending', 'running') LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some(existing_id) = existing {
        return Ok(Json(serde_json::json!({
            "id": existing_id.to_string(),
            "status": "already_running",
        })));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'telegram_sync_incremental', 'pending')",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "pending",
    })))
}

pub async fn start_sync_job(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'telegram_monitor' AND value->>'session_data' IS NOT NULL)",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !session_exists {
        return Err(ApiError::bad_request("Telegram not authorized"));
    }

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id IS NULL AND type = 'telegram_sync' AND status IN ('pending', 'running') LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some(existing_id) = existing {
        return Ok(Json(serde_json::json!({
            "id": existing_id.to_string(),
            "status": "already_running",
        })));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'telegram_sync', 'pending')",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "pending",
    })))
}

// ---------------------------------------------------------------------------
// Background job: process_telegram_sync (called by job poller)
// ---------------------------------------------------------------------------

pub async fn process_telegram_sync(pool: &sqlx::PgPool, job_id: Uuid) -> Result<(), String> {
    info!("[TG_SYNC] Starting telegram sync job {job_id}");

    let (api_id, api_hash, _, session_bytes) = load_tg_settings(pool)
        .await
        .ok_or_else(|| "Telegram not configured".to_string())?;

    let session_bytes = session_bytes
        .ok_or_else(|| "Telegram not authorized".to_string())?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get("library_id")))
    .collect();

    if sources.is_empty() {
        sqlx::query(
            "UPDATE index_jobs SET status = 'success', finished_at = NOW(), \
             stats_json = $2, progress_percent = 100 WHERE id = $1",
        )
        .bind(job_id)
        .bind(serde_json::json!({ "message": "No sources configured", "synced": 0, "new_books": 0 }))
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let r = do_sync(pool.clone(), api_id as i32, api_hash, session_bytes, sources, Some(job_id))
        .await
        .map_err(|e| format!("{e:?}"))?;

    info!("[TG_SYNC] Job {job_id} complete: {} messages, {} new books, {} series searched", r.synced, r.new_books, r.series_searched);

    // Which series matched a local series (current catalog state)
    let matched_series: Vec<serde_json::Value> = sqlx::query(
        "SELECT b.series_name AS telegram_name, s.id AS series_id, s.name AS series_name, \
                COUNT(*) AS book_count \
         FROM telegram_book_links b \
         JOIN telegram_sources src ON src.id = b.source_id \
         JOIN series s ON LOWER(unaccent(s.name)) = LOWER(unaccent(b.series_name)) \
           AND s.library_id = COALESCE(b.library_id, src.library_id) \
         WHERE b.status = 'available' \
         GROUP BY b.series_name, s.id, s.name \
         ORDER BY b.series_name",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .iter()
    .map(|row| serde_json::json!({
        "telegram_name": row.get::<String, _>("telegram_name"),
        "series_id": row.get::<Uuid, _>("series_id").to_string(),
        "series_name": row.get::<String, _>("series_name"),
        "book_count": row.get::<i64, _>("book_count"),
    }))
    .collect();

    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), \
         stats_json = $2, progress_percent = 100 WHERE id = $1",
    )
    .bind(job_id)
    .bind({
        // Merge counts and extracted names for series that appear across multiple sources
        let mut deduped: std::collections::BTreeMap<&str, (usize, std::collections::BTreeSet<String>)> = std::collections::BTreeMap::new();
        for (name, count, extracted) in &r.series_results {
            let entry = deduped.entry(name.as_str()).or_default();
            entry.0 += count;
            entry.1.extend(extracted.iter().cloned());
        }
        let all_series: Vec<serde_json::Value> = deduped.iter().map(|(name, (count, extracted))| serde_json::json!({
            "series_name": name,
            "book_count": count,
            "extracted_names": extracted.iter().collect::<Vec<_>>(),
        })).collect();
        serde_json::json!({
            "synced": r.synced,
            "new_books": r.new_books,
            "series_searched": r.series_searched,
            "all_series": all_series,
            "matched_series": matched_series,
        })
    })
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Background job: process_telegram_sync_incremental (called by job poller)
// ---------------------------------------------------------------------------

pub async fn process_telegram_sync_incremental(pool: &sqlx::PgPool, job_id: Uuid) -> Result<(), String> {
    info!("[TG_SYNC_INC] Starting incremental telegram sync job {job_id}");

    let (api_id, api_hash, _, session_bytes) = load_tg_settings(pool)
        .await
        .ok_or_else(|| "Telegram not configured".to_string())?;

    let session_bytes = session_bytes
        .ok_or_else(|| "Telegram not authorized".to_string())?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get("library_id")))
    .collect();

    if sources.is_empty() {
        sqlx::query(
            "UPDATE index_jobs SET status = 'success', finished_at = NOW(), \
             stats_json = $2, progress_percent = 100 WHERE id = $1",
        )
        .bind(job_id)
        .bind(serde_json::json!({ "message": "No sources configured", "new_books": 0 }))
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let total = sources.len() as i32;
    let _ = sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id).bind(total).execute(pool).await;

    let started_at = chrono::Utc::now();

    let (new_books, sources_scanned, per_source) =
        do_incremental_sync(pool.clone(), api_id as i32, api_hash, session_bytes, sources, job_id)
            .await
            .map_err(|e| format!("{e:?}"))?;

    info!("[TG_SYNC_INC] Job {job_id} complete: {new_books} new books across {sources_scanned} sources");

    // Fetch the books actually added during this run (by creation timestamp)
    let recent_books: Vec<serde_json::Value> = sqlx::query(
        "SELECT b.filename, b.series_name, b.volume_number, s.channel_username \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.created_at >= $1 \
         ORDER BY b.created_at DESC \
         LIMIT 200",
    )
    .bind(started_at)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .iter()
    .map(|r| serde_json::json!({
        "filename":       r.get::<String, _>("filename"),
        "series_name":    r.get::<Option<String>, _>("series_name"),
        "volume_number":  r.get::<Option<i32>, _>("volume_number"),
        "channel":        r.get::<String, _>("channel_username"),
    }))
    .collect();

    let sources_json: Vec<serde_json::Value> = per_source.iter()
        .map(|(username, count)| serde_json::json!({ "username": username, "new_books": count }))
        .collect();

    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), \
         stats_json = $2, progress_percent = 100 WHERE id = $1",
    )
    .bind(job_id)
    .bind(serde_json::json!({
        "new_books": new_books,
        "sources_scanned": sources_scanned,
        "sources": sources_json,
        "recent_books": recent_books,
    }))
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn do_incremental_sync(
    pool: sqlx::PgPool,
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    sources: Vec<(Uuid, String, Option<Uuid>)>,
    job_id: Uuid,
) -> Result<(usize, usize, Vec<(String, usize)>), ApiError> {
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "pdf", "epub", "zip"];

    let session = Session::load(&session_bytes)
        .map_err(|e| ApiError::internal(format!("Session load: {e}")))?;

    let client = Client::connect(Config {
        session,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await
    .map_err(|e| ApiError::internal(format!("Connect: {e}")))?;

    let mut total_new = 0usize;
    let mut sources_scanned = 0usize;
    let mut per_source: Vec<(String, usize)> = Vec::new();
    let total = sources.len() as i32;

    for (idx, (source_id, username, library_id)) in sources.iter().enumerate() {
        let pct = (idx as i32 * 100 / total.max(1)) as i16;
        let label = format!("@{username} (incremental)");
        let _ = sqlx::query(
            "UPDATE index_jobs SET processed_files = $2, progress_percent = $3, current_file = $4 WHERE id = $1",
        )
        .bind(job_id).bind(idx as i32).bind(pct).bind(&label)
        .execute(&pool).await;

        // Highest message_id already stored for this source — scan only newer messages
        let max_known: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(message_id) FROM telegram_book_links WHERE source_id = $1",
        )
        .bind(source_id)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();

        let chat = match client.resolve_username(username).await {
            Ok(Some(c)) => c,
            Ok(None) => { error!("[TG_SYNC_INC] Channel @{username} not found"); continue; }
            Err(e) => { error!("[TG_SYNC_INC] Resolve @{username}: {e}"); continue; }
        };

        let mut new_books = 0usize;
        let mut iter = client.iter_messages(&chat);

        loop {
            match iter.next().await {
                Ok(Some(message)) => {
                    let msg_id = message.id() as i64;
                    if let Some(max_id) = max_known {
                        if msg_id <= max_id {
                            break;
                        }
                    }
                    if let Err(e) = insert_document_message(&pool, *source_id, *library_id, BOOK_EXTENSIONS, &message, &mut new_books).await {
                        error!("[TG_SYNC_INC] Insert @{username} msg {msg_id}: {e}");
                    }
                }
                Ok(None) => break,
                Err(e) => { error!("[TG_SYNC_INC] iter @{username}: {e}"); break; }
            }
        }

        total_new += new_books;
        sources_scanned += 1;
        per_source.push((username.clone(), new_books));
        info!("[TG_SYNC_INC] @{username}: {new_books} new books (since msg id {})", max_known.unwrap_or(0));
    }

    let updated_bytes = client.session().save();
    save_session_to_db(&pool, updated_bytes).await?;

    Ok((total_new, sources_scanned, per_source))
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/channels?q=  — autocomplete from user's dialogs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, ToSchema)]
pub struct ChannelSuggestion {
    pub username: Option<String>,
    pub title: String,
    pub kind: String,
}

#[utoipa::path(
    get, path = "/telegram-monitor/channels",
    tag = "telegram-monitor",
    params(("q" = Option<String>, Query, description = "Search query")),
    responses((status = 200, body = Vec<ChannelSuggestion>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn search_channels(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ChannelSuggestion>>, ApiError> {
    let q = params.get("q").map(|s| s.to_lowercase()).unwrap_or_default();

    let (api_id, api_hash, _, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Not configured"))?;
    let session_bytes = session_bytes
        .ok_or_else(|| ApiError::bad_request("Not authorized"))?;

    let results = do_search_channels(api_id as i32, api_hash, session_bytes, q).await?;
    Ok(Json(results))
}

async fn do_search_channels(
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    query: String,
) -> Result<Vec<ChannelSuggestion>, ApiError> {
    use grammers_client::{Client, Config};
    use grammers_client::types::Chat;
    use grammers_session::Session;

    let session = Session::load(&session_bytes)
        .map_err(|e| ApiError::internal(format!("Session load: {e}")))?;
    let client = Client::connect(Config {
        session,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await
    .map_err(|e| ApiError::internal(format!("Connect: {e}")))?;

    let mut results = Vec::new();
    let mut dialogs = client.iter_dialogs();

    while let Some(dialog) = dialogs.next().await
        .map_err(|e| ApiError::internal(format!("iter dialogs: {e}")))?
    {
        let chat = dialog.chat().clone();
        let title = chat.name().to_string();

        let (username, kind) = match &chat {
            Chat::Channel(c) => (c.username().map(str::to_string), "channel"),
            Chat::Group(_) => (None, "group"),
            Chat::User(_) => continue,
        };

        let matches = query.is_empty()
            || title.to_lowercase().contains(&query)
            || username.as_deref().map(|u| u.to_lowercase().contains(&query)).unwrap_or(false);

        if matches {
            results.push(ChannelSuggestion { username, title, kind: kind.to_string() });
        }

        if results.len() >= 30 {
            break;
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Sources CRUD
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/sources",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramSourceDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramSourceDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, channel_username, channel_title, library_id, enabled, created_at \
         FROM telegram_sources ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?;

    let sources = rows
        .iter()
        .map(|r| TelegramSourceDto {
            id: r.get::<Uuid, _>("id").to_string(),
            channel_username: r.get("channel_username"),
            channel_title: r.get("channel_title"),
            library_id: r.get::<Option<Uuid>, _>("library_id").map(|u| u.to_string()),
            enabled: r.get("enabled"),
            created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        })
        .collect();

    Ok(Json(sources))
}

#[utoipa::path(
    post, path = "/telegram-monitor/sources",
    tag = "telegram-monitor",
    request_body = AddSourceRequest,
    responses((status = 200, body = TelegramSourceDto), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn add_source(
    State(state): State<AppState>,
    Json(body): Json<AddSourceRequest>,
) -> Result<Json<TelegramSourceDto>, ApiError> {
    let username = body.channel_username.trim().trim_start_matches('@').to_string();
    let library_id = body.library_id.as_deref().map(|s| Uuid::parse_str(s)).transpose()
        .map_err(|_| ApiError::bad_request("Invalid library_id"))?;

    let row = sqlx::query(
        "INSERT INTO telegram_sources (channel_username, library_id) VALUES ($1, $2) \
         ON CONFLICT (channel_username) DO UPDATE SET library_id = $2, enabled = true, updated_at = NOW() \
         RETURNING id, channel_username, channel_title, library_id, enabled, created_at",
    )
    .bind(&username)
    .bind(library_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(TelegramSourceDto {
        id: row.get::<Uuid, _>("id").to_string(),
        channel_username: row.get("channel_username"),
        channel_title: row.get("channel_title"),
        library_id: row.get::<Option<Uuid>, _>("library_id").map(|u| u.to_string()),
        enabled: row.get("enabled"),
        created_at: row.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
    }))
}

#[utoipa::path(
    delete, path = "/telegram-monitor/sources/{id}",
    tag = "telegram-monitor",
    params(("id" = String, Path, description = "Source UUID")),
    responses((status = 200), (status = 404), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn delete_source(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let res = sqlx::query("DELETE FROM telegram_sources WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err(ApiError::not_found("source not found"));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/sync  — sync all enabled sources
// ---------------------------------------------------------------------------

#[utoipa::path(
    post, path = "/telegram-monitor/sync",
    tag = "telegram-monitor",
    responses(
        (status = 200, body = SyncResult),
        (status = 400, description = "Not configured or not authorized"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn sync_sources(
    State(state): State<AppState>,
) -> Result<Json<SyncResult>, ApiError> {
    let (api_id, api_hash, _, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Telegram monitor not configured"))?;

    let session_bytes = session_bytes
        .ok_or_else(|| ApiError::bad_request("Not authorized — complete auth first"))?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true",
    )
    .fetch_all(&state.pool)
    .await?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get("library_id")))
    .collect();

    if sources.is_empty() {
        return Ok(Json(SyncResult { synced: 0, new_books: 0, series_searched: 0, series_results: vec![] }));
    }

    let result = do_sync(state.pool.clone(), api_id as i32, api_hash, session_bytes, sources, None).await?;

    Ok(Json(result))
}

async fn do_sync(
    pool: sqlx::PgPool,
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    sources: Vec<(Uuid, String, Option<Uuid>)>,
    job_id: Option<Uuid>,
) -> Result<SyncResult, ApiError> {
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "pdf", "epub", "zip"];

    // Pre-load filtered series per source before opening Telegram connection.
    // Only series with an approved metadata link AND at least one missing volume
    // (same rule as download detection / Prowlarr).
    let mut source_work: Vec<(Uuid, String, Option<Uuid>, Vec<String>)> = Vec::new();
    for (source_id, username, library_id) in sources {
        let series_names: Vec<String> = if let Some(lib_id) = library_id {
            sqlx::query_scalar::<_, String>(
                "SELECT DISTINCT s.name \
                 FROM series s \
                 JOIN external_metadata_links eml \
                   ON eml.series_id = s.id AND eml.status = 'approved' AND eml.library_id = $1 \
                 WHERE s.library_id = $1 \
                   AND EXISTS ( \
                       SELECT 1 FROM external_book_metadata ebm \
                       WHERE ebm.link_id = eml.id AND ebm.book_id IS NULL \
                   ) \
                 ORDER BY s.name",
            )
            .bind(lib_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        } else {
            vec![]
        };

        if series_names.is_empty() {
            info!("Telegram sync @{username}: no eligible series (need metadata link + missing volumes), skipping");
        } else {
            source_work.push((source_id, username, library_id, series_names));
        }
    }

    let total_series = source_work.iter().map(|(_, _, _, sn)| sn.len()).sum::<usize>() as i32;

    if let Some(jid) = job_id {
        let _ = sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
            .bind(jid).bind(total_series).execute(&pool).await;
    }

    let session = Session::load(&session_bytes)
        .map_err(|e| ApiError::internal(format!("Session load: {e}")))?;

    let client = Client::connect(Config {
        session,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await
    .map_err(|e| ApiError::internal(format!("Connect: {e}")))?;

    let mut total_synced = 0usize;
    let mut total_new = 0usize;
    let mut total_series_searched = 0usize;
    let mut all_series_results: Vec<(String, usize, Vec<String>)> = Vec::new();
    let mut processed = 0i32;

    for (source_id, username, library_id, series_names) in source_work {
        total_series_searched += series_names.len();

        match sync_one_source(
            &client, &pool, source_id, &username, library_id,
            BOOK_EXTENSIONS, &series_names,
            job_id, &mut processed, total_series,
        ).await {
            Ok((synced, new_books, series_results)) => {
                total_synced += synced;
                total_new += new_books;
                all_series_results.extend(series_results);
            }
            Err(e) => error!("Telegram sync error for {username}: {e}"),
        }
    }

    let updated_bytes = client.session().save();
    save_session_to_db(&pool, updated_bytes).await?;

    Ok(SyncResult {
        synced: total_synced,
        new_books: total_new,
        series_searched: total_series_searched,
        series_results: all_series_results,
    })
}

async fn insert_document_message(
    pool: &sqlx::PgPool,
    source_id: Uuid,
    library_id: Option<Uuid>,
    extensions: &[&str],
    message: &grammers_client::types::Message,
    new_books: &mut usize,
) -> anyhow::Result<()> {
    use grammers_client::types::Media;

    if let Some(Media::Document(doc)) = message.media() {
        let filename = doc.name().to_string();
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !extensions.contains(&ext.as_str()) {
            return Ok(());
        }

        let msg_id = message.id() as i64;
        let text = message.text();
        let msg_text = if text.is_empty() { None } else { Some(text.to_string()) };
        let series_name = extract_series_name_from_filename(&filename);
        let series_name = if series_name.is_empty() { None } else { Some(series_name) };
        let volume_number = extract_volume(&filename);

        let inserted = sqlx::query(
            "INSERT INTO telegram_book_links \
             (source_id, message_id, filename, file_size, mime_type, message_text, library_id, series_name, volume_number) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (source_id, message_id) DO NOTHING",
        )
        .bind(source_id)
        .bind(msg_id)
        .bind(&filename)
        .bind(doc.size())
        .bind(doc.mime_type())
        .bind(msg_text)
        .bind(library_id)
        .bind(series_name)
        .bind(volume_number)
        .execute(pool)
        .await?;

        if inserted.rows_affected() > 0 {
            *new_books += 1;
        }
    }
    Ok(())
}

async fn sync_one_source(
    client: &grammers_client::Client,
    pool: &sqlx::PgPool,
    source_id: Uuid,
    username: &str,
    library_id: Option<Uuid>,
    extensions: &[&str],
    series_names: &[String],
    job_id: Option<Uuid>,
    processed: &mut i32,
    total: i32,
) -> anyhow::Result<(usize, usize, Vec<(String, usize, Vec<String>)>)> {
    use grammers_client::grammers_tl_types::enums::MessagesFilter;

    let chat = match client.resolve_username(username).await? {
        Some(c) => c,
        None => anyhow::bail!("Channel not found: {username}"),
    };

    use grammers_client::types::Media;
    use std::collections::BTreeSet;

    let mut synced = 0usize;
    let mut new_books = 0usize;
    let mut series_results: Vec<(String, usize, Vec<String>)> = Vec::new();

    for series_name in series_names {
        if let Some(jid) = job_id {
            let pct = (*processed * 100 / total.max(1)) as i16;
            let label = format!("@{username}: {series_name}");
            let _ = sqlx::query(
                "UPDATE index_jobs \
                 SET processed_files = $2, progress_percent = $3, current_file = $4 \
                 WHERE id = $1",
            )
            .bind(jid).bind(*processed).bind(pct).bind(&label)
            .execute(pool).await;
        }

        let mut iter = client
            .search_messages(&chat)
            .query(series_name)
            .filter(MessagesFilter::InputMessagesFilterDocument);

        let mut series_count = 0usize;
        let mut extracted_names: BTreeSet<String> = BTreeSet::new();
        while let Some(message) = iter.next().await? {
            synced += 1;
            series_count += 1;
            if let Some(Media::Document(doc)) = message.media() {
                let extracted = extract_series_name_from_filename(doc.name());
                if !extracted.is_empty() {
                    extracted_names.insert(extracted);
                }
            }
            insert_document_message(pool, source_id, library_id, extensions, &message, &mut new_books).await?;
        }
        if series_count > 0 {
            series_results.push((series_name.clone(), series_count, extracted_names.into_iter().collect()));
        }
        *processed += 1;
    }

    // Update channel title
    if let Some(title) = get_chat_title(client, username).await {
        sqlx::query("UPDATE telegram_sources SET channel_title = $1, updated_at = NOW() WHERE id = $2")
            .bind(title)
            .bind(source_id)
            .execute(pool)
            .await?;
    }

    info!("Telegram search @{username}: {synced} messages scanned, {new_books} new books ({} series with results)", series_results.len());
    Ok((synced, new_books, series_results))
}

async fn get_chat_title(client: &grammers_client::Client, username: &str) -> Option<String> {
    match client.resolve_username(username).await {
        Ok(Some(chat)) => Some(chat.name().to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/books
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/books",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramBookLinkDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_books(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramBookLinkDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT b.id, b.source_id, s.channel_username, b.message_id, b.filename, \
                b.file_size, b.mime_type, b.message_text, b.status, b.library_id, \
                b.book_id, b.error_message, b.series_name, b.volume_number, b.created_at \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.status != 'dismissed' \
         ORDER BY b.created_at DESC \
         LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await?;

    let books = rows
        .iter()
        .map(|r| TelegramBookLinkDto {
            id: r.get::<Uuid, _>("id").to_string(),
            source_id: r.get::<Uuid, _>("source_id").to_string(),
            channel_username: r.get("channel_username"),
            message_id: r.get("message_id"),
            filename: r.get("filename"),
            file_size: r.get("file_size"),
            mime_type: r.get("mime_type"),
            message_text: r.get("message_text"),
            status: r.get("status"),
            library_id: r.get::<Option<Uuid>, _>("library_id").map(|u| u.to_string()),
            book_id: r.get::<Option<Uuid>, _>("book_id").map(|u| u.to_string()),
            error_message: r.get("error_message"),
            series_name: r.get("series_name"),
            volume_number: r.get("volume_number"),
            created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        })
        .collect();

    Ok(Json(books))
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/available  — books grouped by series (for Downloads page)
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/available",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramAvailableGroupDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_available_by_series(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramAvailableGroupDto>>, ApiError> {
    // ROW_NUMBER CTE deduplicates per (source, series, volume): for non-null volumes keeps
    // most recent only; null-volume books each get their own partition via id so all survive.
    let rows = sqlx::query(
        "WITH ranked AS ( \
           SELECT b.id, b.source_id, b.message_id, b.filename, b.file_size, b.status, \
                  b.series_name, b.volume_number, b.created_at, b.library_id, \
                  ROW_NUMBER() OVER ( \
                    PARTITION BY b.source_id, b.series_name, \
                                 COALESCE(b.volume_number::text, b.id::text) \
                    ORDER BY b.created_at DESC \
                  ) AS rn \
           FROM telegram_book_links b \
           WHERE b.status IN ('available', 'failed') AND b.series_name IS NOT NULL \
         ) \
         SELECT r.id, r.source_id, s.channel_username, r.message_id, r.filename, \
                r.file_size, r.status, r.series_name, r.volume_number, r.created_at, \
                COALESCE(r.library_id, s.library_id) AS resolved_library_id \
         FROM ranked r \
         JOIN telegram_sources s ON s.id = r.source_id \
         WHERE r.rn = 1 \
         ORDER BY r.series_name, r.volume_number NULLS LAST, r.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    // Collect unique library IDs and (series_name, library_id) pairs in one pass
    let mut lib_id_set: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    let mut series_key_set: std::collections::HashSet<(String, Uuid)> = std::collections::HashSet::new();
    for r in &rows {
        if let Some(lid) = r.get::<Option<Uuid>, _>("resolved_library_id") {
            lib_id_set.insert(lid);
            if let Some(sn) = r.get::<Option<String>, _>("series_name") {
                series_key_set.insert((sn, lid));
            }
        }
    }
    let lib_ids: Vec<Uuid> = lib_id_set.into_iter().collect();
    let series_keys: Vec<(String, Uuid)> = series_key_set.into_iter().collect();

    // Fetch library names in one query
    let mut lib_names: std::collections::HashMap<Uuid, String> = std::collections::HashMap::new();
    if !lib_ids.is_empty() {
        let libs = sqlx::query("SELECT id, name FROM libraries WHERE id = ANY($1)")
            .bind(&lib_ids)
            .fetch_all(&state.pool)
            .await?;
        for r in &libs {
            lib_names.insert(r.get("id"), r.get("name"));
        }
    }

    // Batch-resolve all (series_name, library_id) → series_id in a single UNNEST query
    let mut series_ids: std::collections::HashMap<(String, Uuid), Option<Uuid>> = std::collections::HashMap::new();
    if !series_keys.is_empty() {
        let (names, libs): (Vec<String>, Vec<Uuid>) = series_keys.iter()
            .map(|(sn, lid)| (sn.clone(), *lid))
            .unzip();
        let batch = sqlx::query(
            "SELECT q.sn AS series_name, q.lid AS library_id, s.id AS series_id \
             FROM UNNEST($1::text[], $2::uuid[]) AS q(sn, lid) \
             LEFT JOIN series s ON s.library_id = q.lid \
               AND LOWER(unaccent(s.name)) = LOWER(unaccent(q.sn))",
        )
        .bind(&names)
        .bind(&libs)
        .fetch_all(&state.pool)
        .await?;
        for r in &batch {
            let sn: String = r.get("series_name");
            let lid: Uuid = r.get("library_id");
            let sid: Option<Uuid> = r.get("series_id");
            series_ids.entry((sn, lid)).or_insert(sid);
        }
        // Ensure every key is present (unmatched series get None)
        for (sn, lid) in &series_keys {
            series_ids.entry((sn.clone(), *lid)).or_insert(None);
        }
    }

    // Batch-fetch owned volumes and series total_volumes for computing missing count
    let matched_series_ids: Vec<Uuid> = series_ids.values()
        .filter_map(|o| *o)
        .collect();
    let mut owned_by_series: std::collections::HashMap<Uuid, Vec<i32>> = std::collections::HashMap::new();
    let mut missing_by_series: std::collections::HashMap<Uuid, i32> = std::collections::HashMap::new();
    if !matched_series_ids.is_empty() {
        // Owned regular volumes per series
        let vol_rows = sqlx::query(
            "SELECT series_id, volume FROM books \
             WHERE series_id = ANY($1) \
               AND volume IS NOT NULL \
               AND volume_type = 'regular' \
             ORDER BY series_id, volume",
        )
        .bind(&matched_series_ids)
        .fetch_all(&state.pool)
        .await?;
        for r in &vol_rows {
            owned_by_series.entry(r.get("series_id")).or_default().push(r.get("volume"));
        }

        // Total volumes per series for missing count
        let total_rows = sqlx::query(
            "SELECT id, total_volumes FROM series WHERE id = ANY($1)",
        )
        .bind(&matched_series_ids)
        .fetch_all(&state.pool)
        .await?;
        for r in &total_rows {
            let sid: Uuid = r.get("id");
            let total: Option<i32> = r.get("total_volumes");
            let owned = owned_by_series.get(&sid).map(|v| v.len()).unwrap_or(0);
            let missing = ((total.unwrap_or(0) as i64) - owned as i64).max(0) as i32;
            missing_by_series.insert(sid, missing);
        }
    }

    // Group books preserving insertion order (SQL ORDER BY series_name)
    let mut group_keys: Vec<(String, Uuid)> = Vec::new();
    let mut groups: std::collections::HashMap<(String, Uuid), Vec<TelegramAvailableBookDto>> = std::collections::HashMap::new();
    for r in &rows {
        let series_name: Option<String> = r.get("series_name");
        let lib_id: Option<Uuid> = r.get("resolved_library_id");
        if let (Some(sn), Some(lid)) = (series_name, lib_id) {
            let key = (sn.clone(), lid);
            if !groups.contains_key(&key) {
                group_keys.push(key.clone());
            }
            let book = TelegramAvailableBookDto {
                id: r.get::<Uuid, _>("id").to_string(),
                channel_username: r.get("channel_username"),
                message_id: r.get("message_id"),
                filename: r.get("filename"),
                file_size: r.get("file_size"),
                volume_number: r.get("volume_number"),
                status: r.get("status"),
                created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
            };
            groups.entry(key).or_default().push(book);
        }
    }

    // Only return groups where the series name was matched to a local series
    let result: Vec<TelegramAvailableGroupDto> = group_keys
        .into_iter()
        .filter_map(|(series_name, lib_id)| {
            let sid_uuid = series_ids.get(&(series_name.clone(), lib_id)).and_then(|o| *o);
            let series_id = sid_uuid.map(|u| u.to_string());
            series_id.as_ref()?; // skip unmatched groups
            let owned_volumes = sid_uuid
                .and_then(|sid| owned_by_series.get(&sid))
                .cloned()
                .unwrap_or_default();
            let series_missing_count = sid_uuid
                .and_then(|sid| missing_by_series.get(&sid))
                .copied()
                .unwrap_or(0);
            let books = groups.remove(&(series_name.clone(), lib_id))?;
            Some(TelegramAvailableGroupDto {
                series_id,
                library_name: lib_names.get(&lib_id).cloned().unwrap_or_default(),
                library_id: lib_id.to_string(),
                owned_volumes,
                series_missing_count,
                series_name,
                books,
            })
        })
        .collect();

    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/downloads  — in-progress / imported / failed books
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/downloads",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramDownloadItemDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_downloads(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramDownloadItemDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT b.id, b.series_name, src.channel_username, b.filename, b.file_size,
                b.volume_number, b.status, b.error_message, b.created_at, b.updated_at,
                COALESCE(b.library_id, src.library_id) AS resolved_library_id,
                l.name AS library_name,
                sr.id AS series_id
         FROM telegram_book_links b
         JOIN telegram_sources src ON src.id = b.source_id
         LEFT JOIN libraries l ON l.id = COALESCE(b.library_id, src.library_id)
         LEFT JOIN series sr ON LOWER(unaccent(sr.name)) = LOWER(unaccent(b.series_name))
           AND sr.library_id = COALESCE(b.library_id, src.library_id)
         WHERE b.status IN ('downloading', 'imported', 'failed')
         ORDER BY b.updated_at DESC
         LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;

    let items = rows.iter().map(|r| TelegramDownloadItemDto {
        id: r.get::<Uuid, _>("id").to_string(),
        series_name: r.get("series_name"),
        series_id: r.get::<Option<Uuid>, _>("series_id").map(|u| u.to_string()),
        library_id: r.get::<Option<Uuid>, _>("resolved_library_id").map(|u| u.to_string()),
        library_name: r.get("library_name"),
        channel_username: r.get("channel_username"),
        filename: r.get("filename"),
        file_size: r.get("file_size"),
        volume_number: r.get("volume_number"),
        status: r.get("status"),
        error_message: r.get("error_message"),
        created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        updated_at: r.get::<chrono::DateTime<Utc>, _>("updated_at").to_rfc3339(),
    }).collect();

    Ok(Json(items))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/books/:id/download
// ---------------------------------------------------------------------------

#[utoipa::path(
    post, path = "/telegram-monitor/books/{id}/download",
    tag = "telegram-monitor",
    params(("id" = String, Path, description = "Book link UUID")),
    request_body = DownloadBookRequest,
    responses(
        (status = 200, description = "Download started"),
        (status = 400, description = "Not configured or not authorized"),
        (status = 404, description = "Book link not found"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn download_book(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
    Json(body): Json<DownloadBookRequest>,
) -> Result<Json<Value>, ApiError> {
    let (api_id, api_hash, _, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Telegram monitor not configured"))?;

    let session_bytes = session_bytes
        .ok_or_else(|| ApiError::bad_request("Not authorized — complete auth first"))?;

    // Resolve library_id: from request, then from the link itself, then from the source
    let override_lib = body.library_id.as_deref()
        .map(|s| Uuid::parse_str(s).ok())
        .flatten();

    let row = sqlx::query(
        "SELECT b.source_id, b.message_id, b.filename, b.status, b.series_name, \
                COALESCE($2::uuid, b.library_id, s.library_id) AS library_id, \
                s.channel_username \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.id = $1",
    )
    .bind(id)
    .bind(override_lib)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("book link not found"))?;

    let status: String = row.get("status");
    if status == "downloading" || status == "imported" {
        return Err(ApiError::bad_request(format!("already {status}")));
    }

    let source_id: Uuid = row.get("source_id");
    let message_id: i64 = row.get("message_id");
    let filename: String = row.get("filename");
    let library_id: Option<Uuid> = row.get("library_id");
    let channel_username: String = row.get("channel_username");
    let series_name: Option<String> = row.get("series_name");

    let library_id = library_id
        .ok_or_else(|| ApiError::bad_request("No library configured for this source"))?;

    // series_name fallback: extract from filename if not stored
    let series_name = series_name
        .unwrap_or_else(|| extract_series_name_from_filename(&filename));

    // Mark as downloading
    sqlx::query(
        "UPDATE telegram_book_links SET status = 'downloading', updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    let pool = state.pool.clone();
    let api_id_i32 = api_id as i32;

    tokio::spawn(async move {
        match do_download(
            pool.clone(),
            api_id_i32,
            api_hash,
            session_bytes,
            id,
            source_id,
            message_id,
            channel_username,
            filename,
            series_name,
            library_id,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("Telegram download failed for {id}: {e}");
                let _ = sqlx::query(
                    "UPDATE telegram_book_links SET status = 'failed', error_message = $1, updated_at = NOW() WHERE id = $2",
                )
                .bind(e.to_string())
                .bind(id)
                .execute(&pool)
                .await;
            }
        }
    });

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[allow(clippy::too_many_arguments)]
async fn do_download(
    pool: sqlx::PgPool,
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    link_id: Uuid,
    _source_id: Uuid,
    message_id: i64,
    channel_username: String,
    filename: String,
    series_name: String,
    library_id: Uuid,
) -> anyhow::Result<()> {
    use grammers_client::{Client, Config};
    use grammers_client::types::Downloadable;
    use grammers_session::Session;

    let session = Session::load(&session_bytes)?;
    let client = Client::connect(Config {
        session,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await?;

    let chat = client
        .resolve_username(&channel_username)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Channel not found: {channel_username}"))?;

    // Find the specific message
    let mut iter = client.iter_messages(&chat).offset_id((message_id + 1) as i32).limit(1);
    let message = iter
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Message {message_id} not found"))?;

    if message.id() as i64 != message_id {
        anyhow::bail!("Message ID mismatch: expected {message_id}, got {}", message.id());
    }

    let media = message
        .media()
        .ok_or_else(|| anyhow::anyhow!("No media in message {message_id}"))?;

    // Resolve target directory using series matching
    let target_dir = resolve_target_dir(&pool, library_id, &series_name).await?;
    std::fs::create_dir_all(&target_dir)?;

    let safe_name = sanitize_filename(&filename);
    let dest_path = std::path::Path::new(&target_dir).join(&safe_name);

    if dest_path.exists() {
        anyhow::bail!("File already exists: {}", dest_path.display());
    }

    let tmp_path = format!("{}.tmp", dest_path.display());

    info!("Downloading {} → {}", filename, dest_path.display());
    let downloadable = Downloadable::Media(media);

    // Retry up to 3 times on FLOOD_WAIT
    const MAX_RETRIES: u32 = 3;
    let mut retries = 0u32;
    loop {
        match client.download_media(&downloadable, &tmp_path).await {
            Ok(_) => break,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("FLOOD_WAIT") && retries < MAX_RETRIES {
                    let wait_secs = parse_flood_wait_secs(&msg).unwrap_or(5);
                    warn!("FLOOD_WAIT {wait_secs}s for {filename}, retry {}/{MAX_RETRIES}", retries + 1);
                    let _ = std::fs::remove_file(&tmp_path);
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs + 1)).await;
                    retries += 1;
                } else {
                    let _ = std::fs::remove_file(&tmp_path);
                    return Err(e.into());
                }
            }
        }
    }

    std::fs::rename(&tmp_path, &dest_path)?;

    let updated_bytes = client.session().save();
    save_session_to_db(&pool, updated_bytes).await
        .map_err(|e| anyhow::anyhow!("session save: {:?}", e))?;

    info!("Downloaded {filename} to {}", dest_path.display());

    // Enqueue scan job
    let scan_job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(scan_job_id)
    .bind(library_id)
    .execute(&pool)
    .await?;

    // Mark imported
    sqlx::query(
        "UPDATE telegram_book_links SET status = 'imported', error_message = NULL, updated_at = NOW() WHERE id = $1",
    )
    .bind(link_id)
    .execute(&pool)
    .await?;

    info!("Telegram download complete: {filename}, scan job {scan_job_id} queued");
    Ok(())
}

fn parse_flood_wait_secs(err: &str) -> Option<u64> {
    // Error format: "rpc error 420: FLOOD_WAIT ... (value: 2)"
    err.split("value:").nth(1)
        .and_then(|s| s.trim().split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse().ok())
}

fn sanitize_filename(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    if safe.is_empty() { "unknown".to_string() } else { safe }
}

// ---------------------------------------------------------------------------
// DELETE /telegram-monitor/books/:id  — dismiss
// ---------------------------------------------------------------------------

#[utoipa::path(
    delete, path = "/telegram-monitor/books/{id}",
    tag = "telegram-monitor",
    params(("id" = String, Path, description = "Book link UUID")),
    responses((status = 200), (status = 404), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn dismiss_book(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let res = sqlx::query(
        "UPDATE telegram_book_links SET status = 'dismissed', updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(ApiError::not_found("book link not found"));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/live-search  — live Telegram search + DB fallback
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct LiveSearchRequest {
    pub query: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TelegramSearchResultDto {
    pub id: String,
    pub channel_username: String,
    pub filename: String,
    pub file_size: Option<i64>,
    pub volume_number: Option<i32>,
    pub series_name: Option<String>,
    pub status: String,
    pub created_at: String,
}

async fn search_books_by_pattern(
    pool: &sqlx::PgPool,
    pattern: &str,
) -> Result<Vec<TelegramSearchResultDto>, ApiError> {
    let rows = sqlx::query(
        "SELECT b.id, s.channel_username, b.filename, b.file_size, b.volume_number, \
                b.series_name, b.status, b.created_at \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.status != 'dismissed' \
           AND b.series_name IS NOT NULL \
           AND LOWER(unaccent(b.series_name)) LIKE LOWER(unaccent($1)) \
         ORDER BY b.volume_number NULLS LAST, b.created_at DESC \
         LIMIT 200",
    )
    .bind(pattern)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| TelegramSearchResultDto {
            id: r.get::<Uuid, _>("id").to_string(),
            channel_username: r.get("channel_username"),
            filename: r.get("filename"),
            file_size: r.get("file_size"),
            volume_number: r.get("volume_number"),
            series_name: r.get("series_name"),
            status: r.get("status"),
            created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        })
        .collect())
}

#[utoipa::path(
    post, path = "/telegram-monitor/live-search",
    tag = "telegram-monitor",
    request_body = LiveSearchRequest,
    responses(
        (status = 200, body = Vec<TelegramSearchResultDto>),
        (status = 400, description = "Not configured or not authorized"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn live_search(
    State(state): State<AppState>,
    Json(body): Json<LiveSearchRequest>,
) -> Result<Json<Vec<TelegramSearchResultDto>>, ApiError> {
    use grammers_client::{Client, Config};
    use grammers_client::grammers_tl_types::enums::MessagesFilter;
    use grammers_session::Session;

    const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "pdf", "epub", "zip"];

    let query = body.query.trim().to_string();
    if query.is_empty() {
        return Err(ApiError::bad_request("Query cannot be empty"));
    }

    let (api_id, api_hash, _phone, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Telegram not configured"))?;

    let session_bytes = session_bytes
        .ok_or_else(|| ApiError::bad_request("Telegram not authorized"))?;

    let session = Session::load(&session_bytes)
        .map_err(|e| ApiError::internal(format!("Session load: {e}")))?;

    let client = Client::connect(Config {
        session,
        api_id: api_id as i32,
        api_hash,
        params: Default::default(),
    })
    .await
    .map_err(|e| ApiError::internal(format!("Telegram connect: {e}")))?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get::<Option<Uuid>, _>("library_id")))
    .collect();

    let mut new_books = 0usize;

    for (source_id, username, library_id) in &sources {
        let chat = match client.resolve_username(username).await {
            Ok(Some(c)) => c,
            Ok(None) => { error!("live_search: channel @{username} not found"); continue; }
            Err(e) => { error!("live_search: resolve @{username}: {e}"); continue; }
        };

        let mut iter = client
            .search_messages(&chat)
            .query(&query)
            .filter(MessagesFilter::InputMessagesFilterDocument);

        loop {
            match iter.next().await {
                Ok(Some(message)) => {
                    if let Err(e) = insert_document_message(&state.pool, *source_id, *library_id, BOOK_EXTENSIONS, &message, &mut new_books).await {
                        error!("live_search insert @{username}: {e}");
                    }
                }
                Ok(None) => break,
                Err(e) => { error!("live_search iter @{username}: {e}"); break; }
            }
        }
    }

    let updated_bytes = client.session().save();
    save_session_to_db(&state.pool, updated_bytes).await?;

    info!("Telegram live search '{query}': {new_books} new books across {} sources", sources.len());

    let pattern = format!("%{}%", query);
    let results = search_books_by_pattern(&state.pool, &pattern).await?;
    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/search?q=...  — local DB search only (fast fallback)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SearchAvailableQuery {
    pub q: Option<String>,
}

#[utoipa::path(
    get, path = "/telegram-monitor/search",
    tag = "telegram-monitor",
    params(("q" = Option<String>, Query, description = "Series name query")),
    responses((status = 200, body = Vec<TelegramSearchResultDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn search_available_books(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<SearchAvailableQuery>,
) -> Result<Json<Vec<TelegramSearchResultDto>>, ApiError> {
    let q = params.q.unwrap_or_default();
    if q.trim().is_empty() {
        return Ok(Json(vec![]));
    }
    let pattern = format!("%{}%", q);
    let results = search_books_by_pattern(&state.pool, &pattern).await?;
    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// AppState field for pending auth
// ---------------------------------------------------------------------------

pub struct PendingAuth {
    pub client: grammers_client::Client,
    pub token: grammers_client::types::LoginToken,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::extract_series_name_from_filename;
    use parsers::extract_volume;

    fn e(filename: &str) -> String {
        extract_series_name_from_filename(filename)
    }

    fn v(filename: &str) -> Option<i32> {
        extract_volume(filename)
    }

    // --- existing patterns ---
    #[test] fn tome_dash() { assert_eq!(e("One Piece - Tome 47.cbz"), "One Piece"); }
    #[test] fn tome_prefix() { assert_eq!(e("Toriko T12.cbz"), "Toriko"); }
    #[test] fn vol_dash() { assert_eq!(e("Naruto - Vol. 3.cbz"), "Naruto"); }
    #[test] fn bare_number() { assert_eq!(e("Berserk 08.cbz"), "Berserk"); }

    // --- chapter markers (the new cases) ---
    #[test] fn ch_dash_number() { assert_eq!(e("Boruto - Two Blue Vortex - Ch11.cbz"), "Boruto - Two Blue Vortex"); }
    #[test] fn ch_dash_lowercase() { assert_eq!(e("Boruto - Two Blue Vortex - ch12.cbz"), "Boruto - Two Blue Vortex"); }
    #[test] fn ch_no_dash() { assert_eq!(e("Boruto - Two Blue Vortex Ch09.cbz"), "Boruto - Two Blue Vortex"); }
    #[test] fn hash_ch_dash() { assert_eq!(e("Boruto - two blue vortex - #Ch03.cbz"), "Boruto - two blue vortex"); }
    #[test] fn hash_ch_no_dash() { assert_eq!(e("Dandadan #Ch05.cbz"), "Dandadan"); }
    #[test] fn hash_digit() { assert_eq!(e("Dandadan #15.cbz"), "Dandadan"); }
    #[test] fn ch_dot() { assert_eq!(e("Gachiakuta - Ch. 38.cbz"), "Gachiakuta"); }

    // --- series names with dashes must NOT be stripped ---
    #[test] fn series_name_with_dash() { assert_eq!(e("Boruto - Two Blue Vortex - Tome 01.cbz"), "Boruto - Two Blue Vortex"); }
    #[test] fn dragon_ball_z() { assert_eq!(e("Dragon Ball Z - Tome 01.cbz"), "Dragon Ball Z"); }

    // --- Telegram @channel attribution ---
    #[test] fn tg_series_channel_tag() { assert_eq!(e("Berserk - 32@BD_fr.cbz"), "Berserk"); }
    #[test] fn tg_volume_channel_tag() { assert_eq!(v("Berserk - 32@BD_fr.cbz"), Some(32)); }
    #[test] fn tg_volume_channel_tag_spaced() { assert_eq!(v("One Piece - 47 @BD_fr.cbz"), Some(47)); }
}
