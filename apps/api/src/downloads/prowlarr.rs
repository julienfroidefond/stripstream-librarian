use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};
use parsers::extract_volumes;

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
#[derive(Debug)]
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
    /// All volumes extracted from the release title (not just missing ones).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_volumes: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrCategory {
    pub id: i32,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
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

/// Returns true if the title indicates a complete/integral edition
/// (e.g., "intégrale", "complet", "complete", "integral").
/// Match a release title against a list of missing volumes.
/// Returns (matched_volumes, all_volumes_in_title).
/// For integral releases, matched_volumes = all missing volumes, all_volumes = empty.
pub(crate) fn match_title_volumes(title: &str, missing_volumes: &[i32]) -> (Vec<i32>, Vec<i32>) {
    let title_volumes = extract_volumes(title);
    let is_integral = is_integral_release(title);

    let matched = if is_integral && !missing_volumes.is_empty() {
        missing_volumes.to_vec()
    } else {
        title_volumes
            .iter()
            .copied()
            .filter(|v| missing_volumes.contains(v))
            .collect()
    };

    let all = if is_integral { vec![] } else { title_volumes };
    (matched, all)
}

pub(crate) fn is_integral_release(title: &str) -> bool {
    let lower = title.to_lowercase();
    // Strip accents for matching: "intégrale" → "integrale"
    let normalized = lower
        .replace(['é', 'è'], "e");
    let keywords = ["integrale", "integral", "complet", "complete", "l'integrale"];
    keywords.iter().any(|kw| {
        // Match as whole word: check boundaries
        normalized.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .any(|word| word == *kw)
    })
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
            let (matched_vols, all_volumes) = match_title_volumes(&r.title, &missing_numbers);
            let matched = if matched_vols.is_empty() { None } else { Some(matched_vols) };

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
                all_volumes,
            }
        })
        .collect()
}

// ─── Inner (testable) functions ──────────────────────────────────────────────

/// Perform a Prowlarr search request against the given base URL.
/// Extracted so it can be called directly in tests (with a wiremock server URL)
/// without needing a DB connection.
async fn do_prowlarr_search(
    base_url: &str,
    api_key: &str,
    query: &str,
    categories: &[i32],
    missing_volumes: Option<&[MissingVolumeInput]>,
) -> Result<ProwlarrSearchResponse, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Stripstream-Librarian")
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let mut params: Vec<(&str, String)> = vec![
        ("query", query.to_string()),
        ("type", "search".to_string()),
    ];
    for cat in categories {
        params.push(("categories", cat.to_string()));
    }

    let resp = client
        .get(format!("{base_url}/api/v1/search"))
        .query(&params)
        .header("X-Api-Key", api_key)
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

    let results = if let Some(missing) = missing_volumes {
        match_missing_volumes(raw_releases, missing)
    } else {
        raw_releases
            .into_iter()
            .map(|r| {
                let all_volumes = extract_volumes(&r.title);
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
                    matched_missing_volumes: None,
                    all_volumes,
                }
            })
            .collect()
    };

    Ok(ProwlarrSearchResponse { results, query: query.to_string() })
}

/// Test the Prowlarr connection against the given base URL.
/// Extracted so it can be called directly in tests (with a wiremock server URL).
async fn do_prowlarr_test(
    base_url: &str,
    api_key: &str,
) -> Result<ProwlarrTestResponse, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Stripstream-Librarian")
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let resp = client
        .get(format!("{base_url}/api/v1/indexer"))
        .header("X-Api-Key", api_key)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let indexers: Vec<serde_json::Value> = r.json().await.unwrap_or_default();
            Ok(ProwlarrTestResponse {
                success: true,
                message: format!("Connected successfully ({} indexers)", indexers.len()),
                indexer_count: Some(indexers.len() as i32),
            })
        }
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            Ok(ProwlarrTestResponse {
                success: false,
                message: format!("Prowlarr returned {status}: {text}"),
                indexer_count: None,
            })
        }
        Err(e) => Ok(ProwlarrTestResponse {
            success: false,
            message: format!("Connection failed: {e}"),
            indexer_count: None,
        }),
    }
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

    let response = do_prowlarr_search(
        &url,
        &api_key,
        &query,
        &categories,
        body.missing_volumes.as_deref(),
    )
    .await?;

    Ok(Json(response))
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

    let response = do_prowlarr_test(&url, &api_key).await?;

    Ok(Json(response))
}

#[cfg(test)]
#[path = "tests/prowlarr.rs"]
mod tests;
