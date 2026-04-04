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
        .replace('é', "e")
        .replace('è', "e");
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
mod tests {
    use super::*;

    fn sorted(mut v: Vec<i32>) -> Vec<i32> {
        v.sort_unstable();
        v
    }

    #[test]
    fn integral_french_accent() {
        assert!(is_integral_release("One Piece - Intégrale [CBZ]"));
        assert!(is_integral_release("Naruto Integrale FR"));
    }

    #[test]
    fn integral_complet() {
        assert!(is_integral_release("Dragon Ball Complet [PDF]"));
        assert!(is_integral_release("Bleach Complete Edition"));
    }

    #[test]
    fn integral_not_false_positive() {
        assert!(!is_integral_release("One Piece T05"));
        assert!(!is_integral_release("Naruto Tome 12"));
        assert!(!is_integral_release("Les Géants - 07 - Moon.cbz"));
        // "intégr" alone is not enough
        assert!(!is_integral_release("Naruto integration test"));
    }

    #[test]
    fn integral_case_insensitive() {
        assert!(is_integral_release("INTEGRALE"));
        assert!(is_integral_release("COMPLET"));
        assert!(is_integral_release("Intégrale"));
    }

    #[test]
    fn integral_l_apostrophe_integrale() {
        assert!(is_integral_release("One Piece - L'intégrale"));
        assert!(is_integral_release("L'INTEGRALE de Naruto"));
    }

    #[test]
    fn integral_with_surrounding_brackets() {
        assert!(is_integral_release("[Intégrale] Bleach"));
        assert!(is_integral_release("Naruto (Complete)"));
    }

    #[test]
    fn integral_partial_word_not_matched() {
        // "completement" contains "complet" but should not match as whole word
        assert!(!is_integral_release("completement different"));
        // "integralement" should not match
        assert!(!is_integral_release("integralement refait"));
    }

    #[test]
    fn match_title_volumes_basic() {
        let (matched, all) = match_title_volumes("One Piece T05", &[3, 5, 7]);
        assert_eq!(matched, vec![5]);
        assert_eq!(all, vec![5]);
    }

    #[test]
    fn match_title_volumes_no_match() {
        let (matched, all) = match_title_volumes("One Piece T05", &[3, 7, 9]);
        assert!(matched.is_empty());
        assert_eq!(all, vec![5]);
    }

    #[test]
    fn match_title_volumes_integral_returns_all_missing() {
        let (matched, all) = match_title_volumes("One Piece Intégrale", &[1, 2, 3, 10, 20]);
        assert_eq!(matched, vec![1, 2, 3, 10, 20]);
        assert!(all.is_empty(), "integral should have empty all_volumes");
    }

    #[test]
    fn match_title_volumes_integral_empty_missing() {
        let (matched, all) = match_title_volumes("One Piece Intégrale", &[]);
        assert!(matched.is_empty());
        assert!(all.is_empty());
    }

    #[test]
    fn match_title_volumes_range_partial_match() {
        let (matched, _all) = match_title_volumes("Dragon Ball T01-T10", &[5, 8, 15]);
        assert_eq!(sorted(matched), vec![5, 8]);
    }

    #[test]
    fn match_missing_volumes_maps_correctly() {
        let releases = vec![
            ProwlarrRawRelease {
                guid: "a".into(),
                title: "Naruto T05".into(),
                size: 100,
                download_url: None,
                indexer: None,
                seeders: None,
                leechers: None,
                publish_date: None,
                protocol: None,
                info_url: None,
                categories: None,
            },
            ProwlarrRawRelease {
                guid: "b".into(),
                title: "Naruto T99".into(),
                size: 200,
                download_url: None,
                indexer: None,
                seeders: None,
                leechers: None,
                publish_date: None,
                protocol: None,
                info_url: None,
                categories: None,
            },
        ];
        let missing = vec![
            MissingVolumeInput { volume_number: Some(5), title: None },
            MissingVolumeInput { volume_number: Some(10), title: None },
        ];
        let result = match_missing_volumes(releases, &missing);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].matched_missing_volumes, Some(vec![5]));
        assert!(result[1].matched_missing_volumes.is_none());
        assert_eq!(result[0].all_volumes, vec![5]);
        assert_eq!(result[1].all_volumes, vec![99]);
    }

    #[test]
    fn match_missing_volumes_with_none_volume() {
        let missing = vec![
            MissingVolumeInput { volume_number: None, title: Some("test".into()) },
        ];
        let releases = vec![
            ProwlarrRawRelease {
                guid: "a".into(),
                title: "Naruto T05".into(),
                size: 100,
                download_url: None,
                indexer: None,
                seeders: None,
                leechers: None,
                publish_date: None,
                protocol: None,
                info_url: None,
                categories: None,
            },
        ];
        let result = match_missing_volumes(releases, &missing);
        // No missing_numbers to match against, so matched should be None
        assert!(result[0].matched_missing_volumes.is_none());
    }

    #[test]
    fn is_integral_with_grave_accent_e() {
        assert!(is_integral_release("Série Intègrale")); // è instead of é
    }

    // ── Wiremock integration tests ──────────────────────────────────────────

    use wiremock::matchers::{method, path, header};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_raw_release(guid: &str, title: &str, size: i64) -> serde_json::Value {
        serde_json::json!({
            "guid": guid,
            "title": title,
            "size": size,
            "downloadUrl": "https://example.com/download/123",
            "indexer": "TestIndexer",
            "seeders": 42,
            "leechers": 5,
            "publishDate": "2025-01-15T10:00:00Z",
            "protocol": "torrent",
            "infoUrl": "https://example.com/info/123",
            "categories": [{"id": 7030, "name": "Comics"}]
        })
    }

    #[tokio::test]
    async fn wiremock_search_returns_releases_with_correct_parsing() {
        let server = MockServer::start().await;

        let body = serde_json::json!([
            make_raw_release("guid-1", "One Piece T05 [FR]", 500_000_000),
            make_raw_release("guid-2", "Naruto Tome 12 [CBZ]", 300_000_000),
        ]);

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .and(header("X-Api-Key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&server)
            .await;

        let result = do_prowlarr_search(
            &server.uri(),
            "test-key",
            "One Piece",
            &[7030],
            None,
        )
        .await
        .expect("search should succeed");

        assert_eq!(result.results.len(), 2);
        assert_eq!(result.query, "One Piece");

        let r0 = &result.results[0];
        assert_eq!(r0.guid, "guid-1");
        assert_eq!(r0.title, "One Piece T05 [FR]");
        assert_eq!(r0.size, 500_000_000);
        assert_eq!(r0.download_url.as_deref(), Some("https://example.com/download/123"));
        assert_eq!(r0.indexer.as_deref(), Some("TestIndexer"));
        assert_eq!(r0.seeders, Some(42));
        assert_eq!(r0.leechers, Some(5));
        assert_eq!(r0.protocol.as_deref(), Some("torrent"));
        assert!(r0.matched_missing_volumes.is_none());
        assert_eq!(r0.all_volumes, vec![5]);

        let r1 = &result.results[1];
        assert_eq!(r1.guid, "guid-2");
        assert_eq!(r1.all_volumes, vec![12]);
    }

    #[tokio::test]
    async fn wiremock_search_with_missing_volumes_matching() {
        let server = MockServer::start().await;

        let body = serde_json::json!([
            make_raw_release("guid-1", "One Piece T05 [FR]", 500_000_000),
            make_raw_release("guid-2", "One Piece T01-T10 [FR]", 1_000_000_000),
            make_raw_release("guid-3", "One Piece T99 [FR]", 200_000_000),
        ]);

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&server)
            .await;

        let missing = vec![
            MissingVolumeInput { volume_number: Some(5), title: None },
            MissingVolumeInput { volume_number: Some(8), title: None },
            MissingVolumeInput { volume_number: Some(15), title: None },
        ];

        let result = do_prowlarr_search(
            &server.uri(),
            "test-key",
            "One Piece",
            &[7030],
            Some(&missing),
        )
        .await
        .expect("search should succeed");

        assert_eq!(result.results.len(), 3);

        // T05 matches missing volume 5
        assert_eq!(result.results[0].matched_missing_volumes, Some(vec![5]));

        // T01-T10 matches missing volumes 5 and 8 (not 15, which is outside range)
        let matched = result.results[1].matched_missing_volumes.as_ref().unwrap();
        let mut sorted_matched = matched.clone();
        sorted_matched.sort();
        assert_eq!(sorted_matched, vec![5, 8]);

        // T99 matches nothing
        assert!(result.results[2].matched_missing_volumes.is_none());
    }

    #[tokio::test]
    async fn wiremock_search_empty_results() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let result = do_prowlarr_search(
            &server.uri(),
            "test-key",
            "Nonexistent Series",
            &[7030, 7020],
            None,
        )
        .await
        .expect("search should succeed");

        assert!(result.results.is_empty());
        assert_eq!(result.query, "Nonexistent Series");
    }

    #[tokio::test]
    async fn wiremock_test_connection_success() {
        let server = MockServer::start().await;

        let indexers = serde_json::json!([
            {"id": 1, "name": "Indexer A"},
            {"id": 2, "name": "Indexer B"},
            {"id": 3, "name": "Indexer C"},
        ]);

        Mock::given(method("GET"))
            .and(path("/api/v1/indexer"))
            .and(header("X-Api-Key", "my-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&indexers))
            .mount(&server)
            .await;

        let result = do_prowlarr_test(&server.uri(), "my-api-key")
            .await
            .expect("test should succeed");

        assert!(result.success);
        assert_eq!(result.indexer_count, Some(3));
        assert!(result.message.contains("3 indexers"));
    }

    #[tokio::test]
    async fn wiremock_test_connection_unauthorized() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/indexer"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let result = do_prowlarr_test(&server.uri(), "bad-key")
            .await
            .expect("test should return a response (not Err)");

        assert!(!result.success);
        assert!(result.indexer_count.is_none());
        assert!(result.message.contains("401"));
    }

    #[tokio::test]
    async fn wiremock_search_http_500_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let err = do_prowlarr_search(
            &server.uri(),
            "test-key",
            "query",
            &[7030],
            None,
        )
        .await
        .unwrap_err();

        assert!(
            err.message.contains("500") || err.message.contains("Internal Server Error"),
            "error should mention 500, got: {}", err.message
        );
    }

    #[tokio::test]
    async fn wiremock_search_invalid_json_response() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&server)
            .await;

        let err = do_prowlarr_search(
            &server.uri(),
            "test-key",
            "query",
            &[7030],
            None,
        )
        .await
        .unwrap_err();

        assert!(
            err.message.contains("parse") || err.message.contains("Parse"),
            "error should mention parse failure, got: {}", err.message
        );
    }

    #[tokio::test]
    async fn wiremock_search_with_integral_release_matching() {
        let server = MockServer::start().await;

        let body = serde_json::json!([
            make_raw_release("guid-int", "One Piece Intégrale [CBZ]", 5_000_000_000_i64),
        ]);

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&server)
            .await;

        let missing = vec![
            MissingVolumeInput { volume_number: Some(1), title: None },
            MissingVolumeInput { volume_number: Some(50), title: None },
            MissingVolumeInput { volume_number: Some(100), title: None },
        ];

        let result = do_prowlarr_search(
            &server.uri(),
            "test-key",
            "One Piece",
            &[7030],
            Some(&missing),
        )
        .await
        .expect("search should succeed");

        assert_eq!(result.results.len(), 1);
        let release = &result.results[0];
        // Integral release should match ALL missing volumes
        let matched = release.matched_missing_volumes.as_ref().unwrap();
        assert_eq!(matched, &vec![1, 50, 100]);
        // all_volumes should be empty for integral releases
        assert!(release.all_volumes.is_empty());
    }
}
