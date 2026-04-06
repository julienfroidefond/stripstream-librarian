use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use crate::metadata_providers::anilist;
use crate::metadata_providers::bedetheque;
use crate::metadata_providers::senscritique;
use crate::state::AppState;

#[cfg(test)]
mod tests;

// ─── DTOs ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct DiscoverySuggestionDto {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub cover_url: Option<String>,
    pub external_url: Option<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct TrendingQuery {
    pub provider: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub period: Option<String>,
    pub nocache: Option<String>,
}

#[derive(Deserialize)]
pub struct AddToLibraryRequest {
    pub library_id: Uuid,
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub authors: Option<Vec<String>>,
    pub publishers: Option<Vec<String>>,
    pub genres: Option<Vec<String>>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    pub status: Option<String>,
    pub cover_url: Option<String>,
    pub external_url: Option<String>,
}

#[derive(Serialize)]
pub struct AddToLibraryResponse {
    pub series_id: Uuid,
    pub metadata_link_id: Uuid,
}

// ─── Cache helpers ──────────────────────────────────────────────────────────

async fn get_cached(pool: &sqlx::PgPool, cache_key: &str) -> Option<Vec<DiscoverySuggestionDto>> {
    let row = sqlx::query(
        "SELECT results FROM discovery_cache WHERE cache_key = $1 AND expires_at > NOW()",
    )
    .bind(cache_key)
    .fetch_optional(pool)
    .await
    .ok()??;

    let results: serde_json::Value = row.get("results");
    serde_json::from_value(results).ok()
}

async fn set_cached(
    pool: &sqlx::PgPool,
    cache_key: &str,
    provider: &str,
    query_type: &str,
    results: &[DiscoverySuggestionDto],
    ttl_hours: i64,
) {
    let results_json = serde_json::to_value(results).unwrap_or_default();
    let _ = sqlx::query(
        r#"
        INSERT INTO discovery_cache (cache_key, provider, query_type, results, fetched_at, expires_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW() + make_interval(hours => $5))
        ON CONFLICT (cache_key)
        DO UPDATE SET results = $4, fetched_at = NOW(), expires_at = NOW() + make_interval(hours => $5)
        "#,
    )
    .bind(cache_key)
    .bind(provider)
    .bind(query_type)
    .bind(results_json)
    .bind(ttl_hours as i32)
    .execute(pool)
    .await;
}

/// Clean up expired cache entries (lazy cleanup)
async fn cleanup_expired(pool: &sqlx::PgPool) {
    let _ = sqlx::query("DELETE FROM discovery_cache WHERE expires_at < NOW()")
        .execute(pool)
        .await;
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// GET /discovery/trending
pub async fn trending(
    State(state): State<AppState>,
    Query(params): Query<TrendingQuery>,
) -> Result<Json<Vec<DiscoverySuggestionDto>>, ApiError> {
    let provider = params.provider.as_deref().unwrap_or("anilist");
    let limit = params.limit.unwrap_or(100).min(200) as usize;
    let offset = params.offset.unwrap_or(0).max(0) as usize;

    let supported = [
        "anilist", "bedetheque",
        "senscritique", "senscritique_bd",
        "sc_trending_bd", "sc_trending_manga",
        "sc_best_bd", "sc_best_manga",
    ];
    if !supported.contains(&provider) {
        return Err(ApiError::bad_request(format!(
            "trending not supported for provider '{provider}'. Supported: {}",
            supported.join(", ")
        )));
    }

    // Period and sort parameters for sc_trending_* providers
    let period = params.period.as_deref().unwrap_or("month");
    let gql_period = match period {
        "week" => "OUTBYWEEK",
        "year" => "OUTBYYEAR",
        _ => "OUTOFMONTH",
    };
    let gql_sort = if provider.starts_with("sc_best") { "RATING" } else { "POPULARITY" };

    // Include period in cache key for providers with period sub-filter
    let cache_key = if provider.starts_with("sc_trending") || provider.starts_with("sc_best") {
        format!("trending:{provider}:{period}")
    } else {
        format!("trending:{provider}")
    };
    let skip_cache = params.nocache.as_deref() == Some("true");

    // Try cache first (unless nocache requested)
    let all_suggestions = if !skip_cache {
        if let Some(cached) = get_cached(&state.pool, &cache_key).await {
            cached
        } else {
            fetch_and_cache_trending(&state.pool, provider, &cache_key, gql_period, gql_sort).await?
        }
    } else {
        fetch_and_cache_trending(&state.pool, provider, &cache_key, gql_period, gql_sort).await?
    };

    // Filter out already-owned, then paginate
    let filtered = filter_already_owned(&state.pool, provider, all_suggestions).await;
    let page = filtered.into_iter().skip(offset).take(limit).collect();

    // Lazy cleanup
    cleanup_expired(&state.pool).await;

    Ok(Json(page))
}

/// Fetch trending from provider and cache the full result set.
async fn fetch_and_cache_trending(
    pool: &sqlx::PgPool,
    provider: &str,
    cache_key: &str,
    gql_period: &str,
    gql_sort: &str,
) -> Result<Vec<DiscoverySuggestionDto>, ApiError> {
    // Fetch maximum from provider
    let fetch_limit = match provider {
        "bedetheque" => 100,
        "senscritique" | "senscritique_bd" | "sc_trending_bd" | "sc_trending_manga" | "sc_best_bd" | "sc_best_manga" => 100,
        _ => 50,
    };
    let candidates = match provider {
        "anilist" => anilist::fetch_trending(fetch_limit)
            .await
            .map_err(|e| ApiError::internal(format!("trending fetch failed: {e}")))?,
        "bedetheque" => bedetheque::fetch_indispensables(None, fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("indispensables fetch failed: {e}")))?,
        "senscritique" => senscritique::fetch_top_mangas(fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("senscritique fetch failed: {e}")))?,
        "senscritique_bd" => senscritique::fetch_top_bd(fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("senscritique_bd fetch failed: {e}")))?,
        "sc_trending_bd" => senscritique::fetch_trending("comicBook", gql_period, gql_sort, Some("BD franco-belge"), fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("sc_trending_bd fetch failed: {e}")))?,
        "sc_trending_manga" => senscritique::fetch_trending("comicBook", gql_period, gql_sort, Some("Manga"), fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("sc_trending_manga fetch failed: {e}")))?,
        "sc_best_bd" => senscritique::fetch_trending("comicBook", gql_period, "RATING", Some("BD franco-belge"), fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("sc_best_bd fetch failed: {e}")))?,
        "sc_best_manga" => senscritique::fetch_trending("comicBook", gql_period, "RATING", Some("Manga"), fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("sc_best_manga fetch failed: {e}")))?,
        _ => unreachable!(),
    };

    let suggestions: Vec<DiscoverySuggestionDto> = candidates
        .into_iter()
        .map(|c| {
            let genres = c
                .metadata_json
                .get("genres")
                .and_then(|g| g.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let status = c
                .metadata_json
                .get("status")
                .and_then(|s| s.as_str())
                .map(String::from);
            DiscoverySuggestionDto {
                provider: provider.to_string(),
                external_id: c.external_id,
                title: c.title,
                authors: c.authors,
                description: c.description,
                genres,
                cover_url: c.cover_url,
                external_url: c.external_url,
                start_year: c.start_year,
                total_volumes: c.total_volumes,
                status,
            }
        })
        .collect();

    // Trending/best providers get 24h TTL; top/poll lists get infinite cache
    let ttl_hours = if provider.starts_with("sc_trending") || provider.starts_with("sc_best") { 24 } else { 87600 };
    set_cached(pool, cache_key, provider, "trending", &suggestions, ttl_hours).await;
    Ok(suggestions)
}

// ─── Prowlarr discovery ─────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ProwlarrDiscoveryItem {
    pub series_name: String,
    pub release_count: usize,
    pub best_seeders: i32,
    pub total_seeders: i32,
    pub categories: Vec<String>,
    pub indexers: Vec<String>,
    pub best_release_title: String,
    pub best_download_url: Option<String>,
    pub best_size: i64,
    pub best_publish_date: Option<String>,
    pub best_info_url: Option<String>,
    pub volumes_found: Vec<i32>,
}

#[derive(Deserialize)]
pub struct ProwlarrDiscoveryQuery {
    pub limit: Option<usize>,
    pub nocache: Option<String>,
}

/// GET /discovery/prowlarr — search Prowlarr indexers and group by series name
pub async fn prowlarr_discovery(
    State(state): State<AppState>,
    Query(params): Query<ProwlarrDiscoveryQuery>,
) -> Result<Json<Vec<ProwlarrDiscoveryItem>>, ApiError> {
    let limit = params.limit.unwrap_or(100).min(200);

    let cache_key = "discovery:prowlarr".to_string();
    let skip_cache = params.nocache.as_deref() == Some("true");

    // Check cache (unless nocache requested)
    if !skip_cache {
        if let Some(cached) = get_cached_raw::<Vec<ProwlarrDiscoveryItem>>(&state.pool, &cache_key).await {
            let filtered = filter_prowlarr_owned(&state.pool, cached).await;
            return Ok(Json(filtered.into_iter().take(limit).collect()));
        }
    }

    // Load Prowlarr config
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'prowlarr'")
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("Prowlarr is not configured"))?;
    let value: serde_json::Value = row.get("value");
    let prowlarr_url = value.get("url").and_then(|u| u.as_str()).unwrap_or("").trim_end_matches('/').to_string();
    let api_key = value.get("api_key").and_then(|k| k.as_str()).unwrap_or("").to_string();
    let categories: Vec<i32> = value.get("categories")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64().map(|n| n as i32)).collect())
        .unwrap_or_else(|| vec![7030, 7020]);

    if prowlarr_url.is_empty() || api_key.is_empty() {
        return Err(ApiError::bad_request("Prowlarr URL and API key must be configured"));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ApiError::internal(format!("HTTP client error: {e}")))?;

    // Search Prowlarr with multiple queries to get a broad set of results.
    // Empty query returns recent releases; named queries find popular series.
    let queries = vec![
        "".to_string(),      // recent releases
        "manga".to_string(),
        "bd".to_string(),
        "comics".to_string(),
        "tome".to_string(),
    ];

    let mut raw: Vec<serde_json::Value> = Vec::new();

    for query in &queries {
        let mut params_vec: Vec<(&str, String)> = vec![
            ("query", query.clone()),
            ("type", "search".to_string()),
        ];
        for cat in &categories {
            params_vec.push(("categories", cat.to_string()));
        }

        let resp = client
            .get(format!("{prowlarr_url}/api/v1/search"))
            .query(&params_vec)
            .header("X-Api-Key", &api_key)
            .send()
            .await;

        if let Ok(resp) = resp {
            if resp.status().is_success() {
                let results: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
                raw.extend(results);
            }
        }

        // Small delay between requests to avoid rate limiting
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    if raw.is_empty() {
        return Ok(Json(vec![]));
    }

    // Group by extracted series name
    let mut series_map: std::collections::HashMap<String, ProwlarrDiscoveryItem> = std::collections::HashMap::new();

    // Deduplicate by GUID to avoid counting the same release from multiple queries
    let mut seen_guids = std::collections::HashSet::new();

    for release in &raw {
        let guid = release.get("guid").and_then(|g| g.as_str()).unwrap_or("").to_string();
        if !guid.is_empty() && !seen_guids.insert(guid) {
            continue; // already processed
        }

        let title = release.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
        let seeders = release.get("seeders").and_then(|s| s.as_i64()).unwrap_or(0) as i32;
        let size = release.get("size").and_then(|s| s.as_i64()).unwrap_or(0);
        let download_url = release.get("downloadUrl").and_then(|u| u.as_str()).map(String::from);
        let indexer = release.get("indexer").and_then(|i| i.as_str()).unwrap_or("").to_string();
        let publish_date = release.get("publishDate").and_then(|d| d.as_str()).map(String::from);
        let info_url = release.get("infoUrl").and_then(|u| u.as_str()).map(String::from);
        let cats: Vec<String> = release.get("categories")
            .and_then(|c| c.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
            .unwrap_or_default();

        // Extract series name from torrent title
        let series_name = extract_series_name_from_torrent(&title);
        if series_name.is_empty() {
            continue;
        }

        // Extract volume numbers from the title
        let volumes = parsers::extract_volumes(&title);

        let key = series_name.to_lowercase();
        let entry = series_map.entry(key).or_insert_with(|| ProwlarrDiscoveryItem {
            series_name: series_name.clone(),
            release_count: 0,
            best_seeders: 0,
            total_seeders: 0,
            categories: Vec::new(),
            indexers: Vec::new(),
            best_release_title: String::new(),
            best_download_url: None,
            best_size: 0,
            best_publish_date: None,
            best_info_url: None,
            volumes_found: Vec::new(),
        });

        entry.release_count += 1;
        entry.total_seeders += seeders;
        if seeders > entry.best_seeders {
            entry.best_seeders = seeders;
            entry.best_release_title = title.clone();
            entry.best_download_url = download_url;
            entry.best_size = size;
            entry.best_publish_date = publish_date;
            entry.best_info_url = info_url;
        }
        for vol in &volumes {
            if !entry.volumes_found.contains(vol) {
                entry.volumes_found.push(*vol);
            }
        }
        for cat in &cats {
            if !entry.categories.contains(cat) {
                entry.categories.push(cat.clone());
            }
        }
        if !indexer.is_empty() && !entry.indexers.contains(&indexer) {
            entry.indexers.push(indexer);
        }
    }

    // Enrich categories by guessing from torrent title when indexer only returns generic categories
    let mut items: Vec<ProwlarrDiscoveryItem> = series_map.into_values().collect();
    for item in &mut items {
        let guessed = guess_category_from_title(&item.best_release_title);
        if let Some(cat) = guessed {
            if !item.categories.contains(&cat) {
                item.categories.insert(0, cat);
            }
        }
    }

    // Sort volumes and sort items by best_seeders descending
    for item in &mut items {
        item.volumes_found.sort_unstable();
    }
    items.sort_by(|a, b| b.best_seeders.cmp(&a.best_seeders));

    tracing::info!("[DISCOVERY] Prowlarr: {} series found from {} raw releases", items.len(), raw.len());

    // Cache for 6 hours
    set_cached_raw(&state.pool, &cache_key, "prowlarr", "discovery", &items, 168).await; // 7 days

    // Filter out already-owned
    let before_filter = items.len();
    let filtered = filter_prowlarr_owned(&state.pool, items).await;
    tracing::info!("[DISCOVERY] Prowlarr: {} after filter (was {})", filtered.len(), before_filter);
    Ok(Json(filtered.into_iter().take(limit).collect()))
}

/// Guess a more specific category from the torrent title keywords.
fn guess_category_from_title(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    // Manga indicators
    if lower.contains("manga") || lower.contains("[jp]") || lower.contains(".jp.")
        || lower.contains("nagatoro") || lower.contains("tome") && lower.contains("ebook")
    {
        return Some("Manga".to_string());
    }
    // Comics indicators
    if lower.contains("comics") || lower.contains("marvel") || lower.contains("dc comics")
        || lower.contains("[en]") || lower.contains(".en.")
    {
        return Some("Comics".to_string());
    }
    // BD indicators (French bande dessinée)
    if lower.contains(" bd ") || lower.contains("[bd]") || lower.contains("-bd-")
        || lower.contains("bande dessinée") || lower.contains("bande dessinee")
        || lower.contains("cbz") || lower.contains("cbr")
    {
        return Some("BD".to_string());
    }
    // French language markers → likely BD
    if lower.contains("[fr]") || lower.contains(".fr.") || lower.contains("/fr")
        || lower.contains("- fr") || lower.ends_with(" fr")
    {
        return Some("BD".to_string());
    }
    None
}

pub fn extract_series_name_from_torrent(title: &str) -> String {
    let lower = title.to_lowercase();
    // Split on common delimiters that separate series name from volume info
    let separators = [" - bd ", " - tome ", " - t0", " - t1", " - t2", " - t3", " - t4", " - t5", " - t6", " - t7", " - t8", " - t9",
        " -bd ", " tome ", " vol.", " vol ", " [", " (", " intégrale", " integrale", " complet"];
    let mut best_pos = title.len();
    for sep in &separators {
        if let Some(pos) = lower.find(sep) {
            if pos > 0 && pos < best_pos {
                best_pos = pos;
            }
        }
    }
    title[..best_pos].trim().to_string()
}

/// Filter out Prowlarr results for series already in the library
async fn filter_prowlarr_owned(
    pool: &sqlx::PgPool,
    items: Vec<ProwlarrDiscoveryItem>,
) -> Vec<ProwlarrDiscoveryItem> {
    if items.is_empty() {
        return items;
    }

    let names: Vec<String> = items.iter().map(|i| i.series_name.to_lowercase()).collect();
    let owned: Vec<String> = sqlx::query_scalar(
        "SELECT LOWER(name) FROM series WHERE LOWER(name) = ANY($1)",
    )
    .bind(&names)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    items
        .into_iter()
        .filter(|i| !owned.contains(&i.series_name.to_lowercase()))
        .collect()
}

// ─── Generic cache helpers for typed data ───────────────────────────────────

async fn get_cached_raw<T: serde::de::DeserializeOwned>(pool: &sqlx::PgPool, cache_key: &str) -> Option<T> {
    let row = sqlx::query(
        "SELECT results FROM discovery_cache WHERE cache_key = $1 AND expires_at > NOW()",
    )
    .bind(cache_key)
    .fetch_optional(pool)
    .await
    .ok()??;

    let results: serde_json::Value = row.get("results");
    serde_json::from_value(results).ok()
}

async fn set_cached_raw<T: serde::Serialize>(
    pool: &sqlx::PgPool,
    cache_key: &str,
    provider: &str,
    query_type: &str,
    results: &T,
    ttl_hours: i64,
) {
    let results_json = serde_json::to_value(results).unwrap_or_default();
    let _ = sqlx::query(
        r#"
        INSERT INTO discovery_cache (cache_key, provider, query_type, results, fetched_at, expires_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW() + make_interval(hours => $5))
        ON CONFLICT (cache_key)
        DO UPDATE SET results = $4, fetched_at = NOW(), expires_at = NOW() + make_interval(hours => $5)
        "#,
    )
    .bind(cache_key)
    .bind(provider)
    .bind(query_type)
    .bind(results_json)
    .bind(ttl_hours as i32)
    .execute(pool)
    .await;
}

/// POST /discovery/add-to-library
pub async fn add_to_library(
    State(state): State<AppState>,
    Json(req): Json<AddToLibraryRequest>,
) -> Result<Json<AddToLibraryResponse>, ApiError> {
    use crate::series::helpers::{CreateSeriesParams, create_series_with_metadata};

    // Normalize provider: sc_trending_bd, sc_best_manga, etc. → senscritique
    let metadata_provider = if req.provider.starts_with("sc_") {
        "senscritique".to_string()
    } else {
        req.provider.clone()
    };

    // Only create metadata links for providers that offer useful data
    let is_linkable = metadata_provider == "bedetheque" || metadata_provider == "senscritique";

    // Build metadata_json with all the discovery-specific fields
    let metadata_json = serde_json::json!({
        "description": req.description,
        "authors": req.authors.as_deref().unwrap_or(&[]),
        "publishers": req.publishers.as_deref().unwrap_or(&[]),
        "genres": req.genres.as_deref().unwrap_or(&[]),
        "start_year": req.start_year,
        "status": req.status,
        "cover_url": req.cover_url,
    });

    let result = create_series_with_metadata(&state, CreateSeriesParams {
        library_id: req.library_id,
        name: req.title.clone(),
        provider: if is_linkable { Some(metadata_provider.clone()) } else { None },
        external_id: if is_linkable { Some(req.external_id.clone()) } else { None },
        external_url: req.external_url.clone(),
        confidence: Some(1.0),
        total_volumes: req.total_volumes,
        metadata_json: Some(metadata_json),
    })
    .await?;

    tracing::info!(
        "[DISCOVERY] Added series '{}' to library {} from provider {}{}",
        req.title,
        req.library_id,
        req.provider,
        if result.metadata_link_id.is_some() { " (metadata link created)" } else { "" }
    );

    Ok(Json(AddToLibraryResponse {
        series_id: result.series_id,
        metadata_link_id: result.metadata_link_id.unwrap_or(result.series_id),
    }))
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Filter out suggestions for series already in any library.
/// Checks both external_metadata_links (by provider+external_id) and series table (by name, case-insensitive).
async fn filter_already_owned(
    pool: &sqlx::PgPool,
    provider: &str,
    suggestions: Vec<DiscoverySuggestionDto>,
) -> Vec<DiscoverySuggestionDto> {
    if suggestions.is_empty() {
        return suggestions;
    }

    // 1. Filter by external_id (exact match via metadata links)
    let external_ids: Vec<String> = suggestions.iter().map(|s| s.external_id.clone()).collect();
    let owned_by_link: Vec<String> = sqlx::query_scalar(
        "SELECT external_id FROM external_metadata_links WHERE provider = $1 AND external_id = ANY($2)",
    )
    .bind(provider)
    .bind(&external_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // 2. Filter by series name (case-insensitive match)
    let titles: Vec<String> = suggestions.iter().map(|s| s.title.to_lowercase()).collect();
    let owned_by_name: Vec<String> = sqlx::query_scalar(
        "SELECT LOWER(name) FROM series WHERE LOWER(name) = ANY($1)",
    )
    .bind(&titles)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    suggestions
        .into_iter()
        .filter(|s| {
            !owned_by_link.contains(&s.external_id)
                && !owned_by_name.contains(&s.title.to_lowercase())
        })
        .collect()
}
