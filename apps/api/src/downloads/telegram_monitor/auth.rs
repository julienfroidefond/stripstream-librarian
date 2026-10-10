use axum::{extract::State, Json};
use serde_json::Value;
use tracing::info;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

use crate::{error::ApiError, state::AppState};
use stripstream_core::settings::load_setting;

use super::types::*;

pub async fn load_tg_settings(
    pool: &sqlx::PgPool,
) -> Option<(i64, String, String, Option<Vec<u8>>)> {
    let v: Value = load_setting(pool, "telegram_monitor")
        .await
        .ok()
        .flatten()?;
    let api_id = v.get("api_id")?.as_i64()?;
    let api_hash = v.get("api_hash")?.as_str()?.to_string();
    let phone = v.get("phone")?.as_str()?.to_string();
    let session_bytes = v
        .get("session_data")
        .and_then(|s| s.as_str())
        .and_then(|s| B64.decode(s).ok());

    Some((api_id, api_hash, phone, session_bytes))
}

pub async fn load_tg_sync_interval(pool: &sqlx::PgPool) -> i32 {
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

pub async fn save_session_to_db(
    pool: &sqlx::PgPool,
    session_bytes: Vec<u8>,
) -> Result<(), ApiError> {
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

    Ok(Json(TelegramMonitorStatus {
        configured,
        authorized,
        phone,
        api_id,
        sync_interval_minutes,
    }))
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
    let existing = load_tg_settings(&state.pool).await;
    let api_id = body
        .api_id
        .or_else(|| existing.as_ref().map(|(id, _, _, _)| *id))
        .ok_or_else(|| ApiError::bad_request("api_id is required"))?;
    let api_hash = body
        .api_hash
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| existing.as_ref().map(|(_, hash, _, _)| hash.clone()))
        .ok_or_else(|| ApiError::bad_request("api_hash is required"))?;
    let phone = body
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| existing.as_ref().map(|(_, _, phone, _)| phone.clone()))
        .ok_or_else(|| ApiError::bad_request("phone is required"))?;

    // Preserve existing session (as base64) if api_id/hash unchanged
    let session_b64 = existing
        .filter(|(id, hash, _, _)| *id == api_id && hash == &api_hash)
        .and_then(|(_, _, _, bytes)| bytes)
        .map(|b| B64.encode(b));

    let sync_interval = body.sync_interval_minutes.unwrap_or(0).max(0);
    let value = serde_json::json!({
        "api_id": api_id,
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
pub async fn start_auth(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
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

    Ok(Json(
        serde_json::json!({ "ok": true, "message": "Code sent to your phone/Telegram app" }),
    ))
}

pub async fn do_start_auth(
    handle: std::sync::Arc<tokio::sync::Mutex<Option<PendingAuth>>>,
    api_id: i32,
    api_hash: String,
    phone: String,
    session_bytes: Option<Vec<u8>>,
) -> Result<(), ApiError> {
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    let session = match session_bytes {
        Some(bytes) => {
            Session::load(&bytes).map_err(|e| ApiError::internal(format!("session load: {e}")))?
        }
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

pub async fn do_verify_auth(
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
pub async fn disconnect(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
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
