use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct MissingVolumeInput {
    pub volume_number: Option<i32>,
    #[allow(dead_code)]
    pub title: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ProwlarrSearchRequest {
    pub series_name: String,
    pub volume_number: Option<i32>,
    pub custom_query: Option<String>,
    pub missing_volumes: Option<Vec<MissingVolumeInput>>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrRawRelease {
    pub guid: String,
    pub title: String,
    pub size: i64,
    pub download_url: Option<String>,
    pub indexer: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub publish_date: Option<String>,
    pub protocol: Option<String>,
    pub info_url: Option<String>,
    pub categories: Option<Vec<ProwlarrCategory>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrRelease {
    pub guid: String,
    pub title: String,
    pub size: i64,
    pub download_url: Option<String>,
    pub indexer: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub publish_date: Option<String>,
    pub protocol: Option<String>,
    pub info_url: Option<String>,
    pub categories: Option<Vec<ProwlarrCategory>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_missing_volumes: Option<Vec<i32>>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrCategory {
    pub id: i32,
    pub name: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ProwlarrSearchResponse {
    pub results: Vec<ProwlarrRelease>,
    pub query: String,
}

#[derive(Serialize, ToSchema)]
pub struct ProwlarrTestResponse {
    pub success: bool,
    pub message: String,
    pub indexer_count: Option<i32>,
}

// ─── Config helper ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ProwlarrConfig {
    url: String,
    api_key: String,
    categories: Option<Vec<i32>>,
}

pub(crate) async fn load_prowlarr_config_internal(
    pool: &sqlx::PgPool,
) -> Result<(String, String, Vec<i32>), ApiError> {
    load_prowlarr_config(pool).await
}

pub(crate) async fn check_prowlarr_configured(pool: &sqlx::PgPool) -> Result<(), ApiError> {
    load_prowlarr_config(pool).await.map(|_| ())
}

pub(crate) fn extract_volumes_from_title_pub(title: &str) -> Vec<i32> {
    extract_volumes_from_title(title)
}

async fn load_prowlarr_config(
    pool: &sqlx::PgPool,
) -> Result<(String, String, Vec<i32>), ApiError> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'prowlarr'")
        .fetch_optional(pool)
        .await?;

    let row = row.ok_or_else(|| ApiError::bad_request("Prowlarr is not configured"))?;
    let value: serde_json::Value = row.get("value");
    let config: ProwlarrConfig = serde_json::from_value(value)
        .map_err(|e| ApiError::internal(format!("invalid prowlarr config: {e}")))?;

    if config.url.is_empty() || config.api_key.is_empty() {
        return Err(ApiError::bad_request(
            "Prowlarr URL and API key must be configured in settings",
        ));
    }

    let url = config.url.trim_end_matches('/').to_string();
    let categories = config.categories.unwrap_or_else(|| vec![7030, 7020]);

    Ok((url, config.api_key, categories))
}

// ─── Volume matching ─────────────────────────────────────────────────────────

/// Extract volume numbers from a release title.
/// Looks for patterns like: T01, Tome 01, Vol. 01, v01, #01,
/// or standalone numbers that appear after common separators.
fn extract_volumes_from_title(title: &str) -> Vec<i32> {
    let lower = title.to_lowercase();
    let mut volumes = Vec::new();

    // Patterns: T01, Tome 01, Tome01, Vol 01, Vol.01, v01, #01
    let prefixes = ["tome", "vol.", "vol ", "t", "v", "#"];
    let chars: Vec<char> = lower.chars().collect();
    let len = chars.len();

    for prefix in &prefixes {
        let mut start = 0;
        while let Some(pos) = lower[start..].find(prefix) {
            let abs_pos = start + pos;
            let after = abs_pos + prefix.len();

            // For single-char prefixes (t, v, #), ensure it's at a word boundary
            if prefix.len() == 1 && *prefix != "#" {
                if abs_pos > 0 && chars[abs_pos - 1].is_alphanumeric() {
                    start = after;
                    continue;
                }
            }

            // Skip optional spaces after prefix
            let mut i = after;
            while i < len && chars[i] == ' ' {
                i += 1;
            }

            // Read digits
            let digit_start = i;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }

            if i > digit_start {
                if let Ok(num) = lower[digit_start..i].parse::<i32>() {
                    if !volumes.contains(&num) {
                        volumes.push(num);
                    }
                }
            }

            start = after;
        }
    }

    volumes
}

/// Match releases against missing volume numbers.
fn match_missing_volumes(
    releases: Vec<ProwlarrRawRelease>,
    missing: &[MissingVolumeInput],
) -> Vec<ProwlarrRelease> {
    let missing_numbers: Vec<i32> = missing
        .iter()
        .filter_map(|m| m.volume_number)
        .collect();

    releases
        .into_iter()
        .map(|r| {
            let matched = if missing_numbers.is_empty() {
                None
            } else {
                let title_volumes = extract_volumes_from_title(&r.title);
                let matched: Vec<i32> = title_volumes
                    .into_iter()
                    .filter(|v| missing_numbers.contains(v))
                    .collect();
                if matched.is_empty() {
                    None
                } else {
                    Some(matched)
                }
            };

            ProwlarrRelease {
                guid: r.guid,
                title: r.title,
                size: r.size,
                download_url: r.download_url,
                indexer: r.indexer,
                seeders: r.seeders,
                leechers: r.leechers,
                publish_date: r.publish_date,
                protocol: r.protocol,
                info_url: r.info_url,
                categories: r.categories,
                matched_missing_volumes: matched,
            }
        })
        .collect()
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Search for releases on Prowlarr
#[utoipa::path(
    post,
    path = "/prowlarr/search",
    tag = "prowlarr",
    request_body = ProwlarrSearchRequest,
    responses(
        (status = 200, body = ProwlarrSearchResponse),
        (status = 400, description = "Bad request or Prowlarr not configured"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Prowlarr connection error"),
    ),
    security(("Bearer" = []))
)]
pub async fn search_prowlarr(
    State(state): State<AppState>,
    Json(body): Json<ProwlarrSearchRequest>,
) -> Result<Json<ProwlarrSearchResponse>, ApiError> {
    let (url, api_key, categories) = load_prowlarr_config(&state.pool).await?;

    let query = if let Some(custom) = &body.custom_query {
        custom.clone()
    } else if let Some(vol) = body.volume_number {
        format!("\"{}\" {}", body.series_name, vol)
    } else {
        format!("\"{}\"", body.series_name)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let mut params: Vec<(&str, String)> = vec![
        ("query", query.clone()),
        ("type", "search".to_string()),
    ];
    for cat in &categories {
        params.push(("categories", cat.to_string()));
    }

    let resp = client
        .get(format!("{url}/api/v1/search"))
        .query(&params)
        .header("X-Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Prowlarr request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::internal(format!(
            "Prowlarr returned {status}: {text}"
        )));
    }

    let raw_text = resp
        .text()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to read Prowlarr response: {e}")))?;

    tracing::debug!("Prowlarr raw response length: {} chars", raw_text.len());

    let raw_releases: Vec<ProwlarrRawRelease> = serde_json::from_str(&raw_text)
        .map_err(|e| {
            tracing::error!("Failed to parse Prowlarr response: {e}");
            tracing::error!("Raw response (first 500 chars): {}", &raw_text[..raw_text.len().min(500)]);
            ApiError::internal(format!("Failed to parse Prowlarr response: {e}"))
        })?;

    let results = if let Some(missing) = &body.missing_volumes {
        match_missing_volumes(raw_releases, missing)
    } else {
        raw_releases
            .into_iter()
            .map(|r| ProwlarrRelease {
                guid: r.guid,
                title: r.title,
                size: r.size,
                download_url: r.download_url,
                indexer: r.indexer,
                seeders: r.seeders,
                leechers: r.leechers,
                publish_date: r.publish_date,
                protocol: r.protocol,
                info_url: r.info_url,
                categories: r.categories,
                matched_missing_volumes: None,
            })
            .collect()
    };

    Ok(Json(ProwlarrSearchResponse { results, query }))
}

/// Test connection to Prowlarr
#[utoipa::path(
    get,
    path = "/prowlarr/test",
    tag = "prowlarr",
    responses(
        (status = 200, body = ProwlarrTestResponse),
        (status = 400, description = "Prowlarr not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn test_prowlarr(
    State(state): State<AppState>,
) -> Result<Json<ProwlarrTestResponse>, ApiError> {
    let (url, api_key, _categories) = load_prowlarr_config(&state.pool).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let resp = client
        .get(format!("{url}/api/v1/indexer"))
        .header("X-Api-Key", &api_key)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let indexers: Vec<serde_json::Value> = r.json().await.unwrap_or_default();
            Ok(Json(ProwlarrTestResponse {
                success: true,
                message: format!("Connected successfully ({} indexers)", indexers.len()),
                indexer_count: Some(indexers.len() as i32),
            }))
        }
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            Ok(Json(ProwlarrTestResponse {
                success: false,
                message: format!("Prowlarr returned {status}: {text}"),
                indexer_count: None,
            }))
        }
        Err(e) => Ok(Json(ProwlarrTestResponse {
            success: false,
            message: format!("Connection failed: {e}"),
            indexer_count: None,
        })),
    }
}
