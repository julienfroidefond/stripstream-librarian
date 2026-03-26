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

    let is_managed = body.library_id.is_some()
        && body.series_name.is_some()
        && body.expected_volumes.is_some();

    let (base_url, username, password) = load_qbittorrent_config(&state.pool).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let sid = qbittorrent_login(&client, &base_url, &username, &password).await?;

    let mut form_params: Vec<(&str, &str)> = vec![("urls", &body.url)];
    let savepath = "/downloads";
    if is_managed {
        form_params.push(("savepath", savepath));
    }

    let resp = client
        .post(format!("{base_url}/api/v2/torrents/add"))
        .header("Cookie", format!("SID={sid}"))
        .form(&form_params)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("qBittorrent add request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Ok(Json(QBittorrentAddResponse {
            success: false,
            message: format!("qBittorrent returned {status}: {text}"),
            torrent_download_id: None,
        }));
    }

    // If managed download: record in torrent_downloads
    let torrent_download_id = if is_managed {
        let library_id = body.library_id.unwrap();
        let series_name = body.series_name.as_deref().unwrap();
        let expected_volumes = body.expected_volumes.as_deref().unwrap();
        let qb_hash = extract_magnet_hash(&body.url);

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, qb_hash) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(library_id)
        .bind(series_name)
        .bind(expected_volumes)
        .bind(qb_hash.as_deref())
        .execute(&state.pool)
        .await?;

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

/// Extract the info-hash from a magnet link (lowercased, hex or base32).
/// magnet:?xt=urn:btih:HASH...
fn extract_magnet_hash(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    let marker = "urn:btih:";
    let start = lower.find(marker)? + marker.len();
    let hash_part = &lower[start..];
    let end = hash_part
        .find(|c: char| !c.is_alphanumeric())
        .unwrap_or(hash_part.len());
    let hash = &hash_part[..end];
    if hash.is_empty() { None } else { Some(hash.to_string()) }
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
