use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use stripstream_core::http::build_http_client_with_agent;
use stripstream_core::settings::load_setting;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};
use parsers::{extract_volumes, match_release_title};

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

#[derive(Serialize, ToSchema, Debug)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_confidence: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub match_reasons: Vec<String>,
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
    /// Machine-readable warning. `indexers_not_synced` means Prowlarr returned
    /// no result on any attempt: its indexers may still be syncing (typical
    /// right after a restart).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Warning code returned when Prowlarr answered every search attempt with `[]`.
pub(crate) const WARNING_INDEXERS_NOT_SYNCED: &str = "indexers_not_synced";

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
    /// Defaults to enabled so configurations saved before the flag existed keep working.
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

pub(crate) async fn load_prowlarr_config_internal(
    pool: &sqlx::PgPool,
) -> Result<(String, String, Vec<i32>), ApiError> {
    load_prowlarr_config(pool).await
}

pub(crate) async fn check_prowlarr_configured(pool: &sqlx::PgPool) -> Result<(), ApiError> {
    load_prowlarr_config(pool).await.map(|_| ())
}

async fn load_prowlarr_config(pool: &sqlx::PgPool) -> Result<(String, String, Vec<i32>), ApiError> {
    let config = load_setting::<ProwlarrConfig>(pool, "prowlarr")
        .await
        .map_err(|e| ApiError::internal(format!("invalid prowlarr config: {e}")))?
        .ok_or_else(|| ApiError::bad_request("Prowlarr is not configured"))?;

    if !config.enabled {
        return Err(ApiError::bad_request("Prowlarr is disabled"));
    }

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
    series_name: &str,
    missing: &[MissingVolumeInput],
) -> Vec<ProwlarrRelease> {
    let missing_numbers: Vec<i32> = missing.iter().filter_map(|m| m.volume_number).collect();

    releases
        .into_iter()
        .map(|r| {
            let title_match = match_release_title(&r.title, series_name, &missing_numbers);
            let (matched, all_volumes, match_confidence, match_reasons) = title_match.map_or_else(
                || (None, extract_volumes(&r.title), None, vec![]),
                |matched| {
                    (
                        Some(matched.matched_missing_volumes),
                        matched.all_volumes,
                        Some(matched.confidence.as_str().to_string()),
                        matched
                            .reasons
                            .into_iter()
                            .map(|reason| reason.as_str().to_string())
                            .collect(),
                    )
                },
            );

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
                match_confidence,
                match_reasons,
            }
        })
        .collect()
}

// ─── Inner (testable) functions ──────────────────────────────────────────────

/// Result of a Prowlarr search, including whether the category-less fallback
/// had to be used.
pub(crate) struct ProwlarrSearchOutcome {
    pub releases: Vec<ProwlarrRawRelease>,
    /// True when the configured category filter returned nothing and the
    /// releases (if any) come from an unfiltered search.
    pub used_category_fallback: bool,
    /// True when the unfiltered fallback returned releases before local
    /// category filtering. Lets callers distinguish "Prowlarr is empty" from
    /// "releases exist but none match the configured categories".
    pub fallback_saw_results: bool,
}

/// Single `/api/v1/search` request with an optional category filter.
async fn request_releases(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    query: &str,
    categories: &[i32],
) -> Result<Vec<ProwlarrRawRelease>, String> {
    let mut params: Vec<(&str, String)> =
        vec![("query", query.to_string()), ("type", "search".to_string())];
    for cat in categories {
        params.push(("categories", cat.to_string()));
    }

    let resp = client
        .get(format!("{base_url}/api/v1/search"))
        .query(&params)
        .header("X-Api-Key", api_key)
        .send()
        .await
        .map_err(|e| format!("Prowlarr request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Prowlarr returned {status}: {text}"));
    }

    let raw_text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read Prowlarr response: {e}"))?;

    serde_json::from_str(&raw_text).map_err(|e| {
        let preview: String = raw_text.chars().take(500).collect();
        tracing::error!("Failed to parse Prowlarr response: {e}");
        tracing::error!("Raw response (first 500 chars): {preview}");
        format!("Failed to parse Prowlarr response: {e}")
    })
}

/// Check a release against the configured Newznab categories.
///
/// Releases without categories are kept, matching Prowlarr's own post-filter
/// (it cannot know their category either). A standard parent category such as
/// `7000` matches its `70xx` children, as Prowlarr expands parents when
/// filtering.
fn release_matches_categories(release: &ProwlarrRawRelease, categories: &[i32]) -> bool {
    if categories.is_empty() {
        return true;
    }
    let Some(release_categories) = release
        .categories
        .as_deref()
        .filter(|categories| !categories.is_empty())
    else {
        return true;
    };

    release_categories
        .iter()
        .any(|release_category| category_matches(release_category.id, categories))
}

fn category_matches(release_category: i32, configured: &[i32]) -> bool {
    configured.iter().any(|&category| {
        release_category == category
            || ((1000..10000).contains(&category)
                && category % 1000 == 0
                && (1000..10000).contains(&release_category)
                && release_category / 1000 == category / 1000)
    })
}

/// Search Prowlarr for a query, falling back to an unfiltered search when the
/// category filter yields nothing.
///
/// On a cold Prowlarr (just restarted), indexer capabilities may not be loaded
/// yet: Prowlarr then drops every indexer whose categories are unknown and
/// returns `200 []` for a category-filtered search. Sonarr/Radarr avoid this by
/// going through the per-indexer Torznab proxy; we retry without `categories`
/// so the first search after a Prowlarr restart returns the same results as its
/// UI instead of requiring a manual warm-up search.
///
/// `retry_delay` controls the extra attempt after an empty unfiltered search;
/// background detection passes `None` to avoid sleeping on every series that
/// genuinely has no release.
pub(crate) async fn fetch_releases(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    query: &str,
    categories: &[i32],
    retry_delay: Option<std::time::Duration>,
) -> Result<ProwlarrSearchOutcome, String> {
    let filtered = request_releases(client, base_url, api_key, query, categories).await?;
    if !filtered.is_empty() || categories.is_empty() {
        return Ok(ProwlarrSearchOutcome {
            releases: filtered,
            used_category_fallback: false,
            fallback_saw_results: false,
        });
    }

    tracing::warn!(
        "[PROWLARR] Empty results for query '{query}' with categories {categories:?}; retrying without the category filter (indexer capabilities may not be synced yet)"
    );
    let unfiltered = request_releases(client, base_url, api_key, query, &[]).await?;
    let fallback_saw_results = !unfiltered.is_empty();
    let unfiltered: Vec<_> = unfiltered
        .into_iter()
        .filter(|release| release_matches_categories(release, categories))
        .collect();
    if !unfiltered.is_empty() {
        tracing::info!(
            "[PROWLARR] Category-less fallback returned {} results for query '{query}'",
            unfiltered.len()
        );
        return Ok(ProwlarrSearchOutcome {
            releases: unfiltered,
            used_category_fallback: true,
            fallback_saw_results,
        });
    }

    let Some(delay) = retry_delay else {
        return Ok(ProwlarrSearchOutcome {
            releases: Vec::new(),
            used_category_fallback: true,
            fallback_saw_results,
        });
    };

    // Prowlarr sometimes answers [] transiently while indexers are syncing.
    tokio::time::sleep(delay).await;
    let retried = request_releases(client, base_url, api_key, query, &[]).await?;
    let fallback_saw_results = fallback_saw_results || !retried.is_empty();
    Ok(ProwlarrSearchOutcome {
        releases: retried
            .into_iter()
            .filter(|release| release_matches_categories(release, categories))
            .collect(),
        used_category_fallback: true,
        fallback_saw_results,
    })
}

/// Perform a Prowlarr search request against the given base URL.
/// Extracted so it can be called directly in tests (with a wiremock server URL)
/// without needing a DB connection.
async fn do_prowlarr_search(
    base_url: &str,
    api_key: &str,
    query: &str,
    series_name: &str,
    categories: &[i32],
    missing_volumes: Option<&[MissingVolumeInput]>,
) -> Result<ProwlarrSearchResponse, ApiError> {
    let client = build_http_client_with_agent(std::time::Duration::from_secs(60))
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let outcome = fetch_releases(
        &client,
        base_url,
        api_key,
        query,
        categories,
        Some(std::time::Duration::from_secs(2)),
    )
    .await
    .map_err(ApiError::internal)?;

    tracing::info!(
        "[PROWLARR] Search '{}' returned {} results{}",
        query,
        outcome.releases.len(),
        if outcome.used_category_fallback {
            " (category-less fallback)"
        } else {
            ""
        }
    );

    let warning = (outcome.releases.is_empty() && !outcome.fallback_saw_results)
        .then(|| WARNING_INDEXERS_NOT_SYNCED.to_string());

    let results = if let Some(missing) = missing_volumes {
        match_missing_volumes(outcome.releases, series_name, missing)
    } else {
        outcome
            .releases
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
                    match_confidence: None,
                    match_reasons: vec![],
                }
            })
            .collect()
    };

    Ok(ProwlarrSearchResponse {
        results,
        query: query.to_string(),
        warning,
    })
}

/// Test the Prowlarr connection against the given base URL.
/// Extracted so it can be called directly in tests (with a wiremock server URL).
async fn do_prowlarr_test(base_url: &str, api_key: &str) -> Result<ProwlarrTestResponse, ApiError> {
    let client = build_http_client_with_agent(std::time::Duration::from_secs(10))
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
        &body.series_name,
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
