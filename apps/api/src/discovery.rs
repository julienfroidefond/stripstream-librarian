use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use crate::metadata_providers::anilist;
use crate::metadata_providers::bedetheque;
use crate::metadata_providers::senscritique;
use crate::series::get_or_create_series;
use crate::state::AppState;

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
    // 1. Create or get the series
    let series_id = get_or_create_series(&state.pool, req.library_id, &req.title).await?;

    // 2. Update series metadata
    let authors = req.authors.unwrap_or_default();
    let publishers = req.publishers.unwrap_or_default();
    let genres = req.genres.unwrap_or_default();

    sqlx::query(
        r#"
        UPDATE series SET
            description = COALESCE($2, description),
            authors = CASE WHEN array_length($3::text[], 1) > 0 THEN $3 ELSE authors END,
            publishers = CASE WHEN array_length($4::text[], 1) > 0 THEN $4 ELSE publishers END,
            genres = CASE WHEN array_length($5::text[], 1) > 0 THEN $5 ELSE genres END,
            start_year = COALESCE($6, start_year),
            total_volumes = COALESCE($7, total_volumes),
            status = COALESCE($8, status),
            cover_url = COALESCE($9, cover_url),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(series_id)
    .bind(&req.description)
    .bind(&authors)
    .bind(&publishers)
    .bind(&genres)
    .bind(req.start_year)
    .bind(req.total_volumes)
    .bind(&req.status)
    .bind(&req.cover_url)
    .execute(&state.pool)
    .await?;

    // 3. Create external_metadata_link (approved) — only for providers that
    //    offer useful metadata links (skip prowlarr, anilist, etc.)
    // Normalize provider name: sc_trending_bd, sc_best_manga, etc. → senscritique
    let metadata_provider = if req.provider.starts_with("sc_") {
        "senscritique".to_string()
    } else {
        req.provider.clone()
    };
    let is_linkable_provider = metadata_provider == "bedetheque" || metadata_provider == "senscritique";
    let link_id: Option<Uuid> = if is_linkable_provider {
        let id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO external_metadata_links
                (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external, matched_at, approved_at, synced_at)
            VALUES ($1, $2, $3, $4, $5, 'approved', 1.0, $6, $7, NOW(), NOW(), NOW())
            ON CONFLICT (series_id, provider) DO UPDATE SET
                external_id = EXCLUDED.external_id,
                external_url = EXCLUDED.external_url,
                status = 'approved',
                total_volumes_external = EXCLUDED.total_volumes_external,
                approved_at = NOW(),
                synced_at = NOW()
            RETURNING id
            "#,
        )
        .bind(req.library_id)
        .bind(series_id)
        .bind(&metadata_provider)
        .bind(&req.external_id)
        .bind(&req.external_url)
        .bind(serde_json::json!({
            "genres": genres,
            "status": req.status,
        }))
        .bind(req.total_volumes)
        .fetch_one(&state.pool)
        .await?;
        Some(id)
    } else {
        None
    };

    tracing::info!(
        "[DISCOVERY] Added series '{}' to library {} from provider {}{}",
        req.title,
        req.library_id,
        req.provider,
        if link_id.is_some() { " (metadata link created)" } else { "" }
    );

    Ok(Json(AddToLibraryResponse {
        series_id,
        metadata_link_id: link_id.unwrap_or(series_id),
    }))
}

// ─── Helpers ────────────────────────────────────────────────────────────────

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::Json;
    use lru::LruCache;
    use std::num::NonZeroUsize;
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::sync::{Mutex, RwLock, Semaphore};

    use crate::state::{AppState, DynamicSettings, Metrics, ReadRateLimit};

    /// Build a minimal AppState suitable for testing (only `pool` is used).
    fn test_state(pool: sqlx::PgPool) -> AppState {
        AppState {
            pool,
            bootstrap_token: Arc::from("test-token"),
            page_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1).unwrap()))),
            page_render_limit: Arc::new(Semaphore::new(1)),
            metrics: Arc::new(Metrics {
                requests_total: AtomicU64::new(0),
                page_cache_hits: AtomicU64::new(0),
                page_cache_misses: AtomicU64::new(0),
            }),
            read_rate_limit: Arc::new(Mutex::new(ReadRateLimit {
                window_started_at: Instant::now(),
                requests_in_window: 0,
            })),
            settings: Arc::new(RwLock::new(DynamicSettings::default())),
        }
    }

    /// Insert a test library and return its UUID.
    async fn create_test_library(pool: &sqlx::PgPool) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind("Test Library")
            .bind(format!("/test/{}", id))
            .execute(pool)
            .await
            .expect("failed to create test library");
        id
    }

    fn make_request(library_id: Uuid, provider: &str, external_id: &str, title: &str) -> AddToLibraryRequest {
        AddToLibraryRequest {
            library_id,
            provider: provider.to_string(),
            external_id: external_id.to_string(),
            title: title.to_string(),
            description: Some("A test description".to_string()),
            authors: Some(vec!["Author A".to_string()]),
            publishers: None,
            genres: Some(vec!["Action".to_string()]),
            start_year: Some(2020),
            total_volumes: Some(10),
            status: Some("ongoing".to_string()),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
            external_url: Some("https://example.com/series/123".to_string()),
        }
    }

    // 1. Basic add from bedetheque — series + metadata link created
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn test_add_to_library_basic_bedetheque(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool).await;
        let state = test_state(pool.clone());
        let req = make_request(library_id, "bedetheque", "ext-123", "Test BD Series");

        let result = add_to_library(State(state), Json(req)).await;
        assert!(result.is_ok(), "add_to_library should succeed");

        let resp = result.unwrap().0;

        // Verify series was created
        let series_row = sqlx::query("SELECT name, description, start_year, total_volumes, status FROM series WHERE id = $1")
            .bind(resp.series_id)
            .fetch_one(&pool)
            .await
            .expect("series should exist");
        let name: String = series_row.get("name");
        assert_eq!(name, "Test BD Series");
        let desc: Option<String> = series_row.get("description");
        assert_eq!(desc, Some("A test description".to_string()));
        let start_year: Option<i32> = series_row.get("start_year");
        assert_eq!(start_year, Some(2020));

        // Verify metadata link was created
        let link_row = sqlx::query(
            "SELECT provider, external_id, status FROM external_metadata_links WHERE series_id = $1 AND provider = 'bedetheque'"
        )
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .expect("metadata link should exist");
        let provider: String = link_row.get("provider");
        assert_eq!(provider, "bedetheque");
        let ext_id: String = link_row.get("external_id");
        assert_eq!(ext_id, "ext-123");
        let status: String = link_row.get("status");
        assert_eq!(status, "approved");
    }

    // 2. Duplicate series — same series_id returned, no duplicate
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn test_add_to_library_duplicate_series(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool).await;
        let state = test_state(pool.clone());

        let req1 = make_request(library_id, "bedetheque", "ext-100", "Duplicate Series");
        let resp1 = add_to_library(State(state.clone()), Json(req1)).await.unwrap().0;

        let req2 = make_request(library_id, "bedetheque", "ext-200", "Duplicate Series");
        let resp2 = add_to_library(State(state), Json(req2)).await.unwrap().0;

        // Same series_id should be returned
        assert_eq!(resp1.series_id, resp2.series_id, "should return the same series_id for duplicate name");

        // Only one series row
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM series WHERE library_id = $1 AND name = 'Duplicate Series'"
        )
        .bind(library_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "should have exactly one series row");

        // Metadata link should have been upserted (ON CONFLICT updates external_id)
        let link_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1 AND provider = 'bedetheque'"
        )
        .bind(resp1.series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(link_count, 1, "should have exactly one metadata link per (series_id, provider)");
    }

    // 3. Non-linkable provider (anilist) — no metadata link created
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn test_add_to_library_anilist_no_metadata_link(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool).await;
        let state = test_state(pool.clone());
        let req = make_request(library_id, "anilist", "ani-456", "Anilist Series");

        let resp = add_to_library(State(state), Json(req)).await.unwrap().0;

        // Series should exist
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE id = $1")
            .bind(resp.series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "series should be created");

        // No metadata link should be created
        let link_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1"
        )
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(link_count, 0, "anilist provider should not create metadata link");

        // metadata_link_id should fall back to series_id
        assert_eq!(resp.metadata_link_id, resp.series_id, "metadata_link_id should equal series_id when no link created");
    }

    // 4. SC provider normalization — sc_trending_bd → senscritique
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn test_add_to_library_sc_provider_normalization(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool).await;
        let state = test_state(pool.clone());
        let req = make_request(library_id, "sc_trending_bd", "sc-789", "SC Trending Series");

        let resp = add_to_library(State(state), Json(req)).await.unwrap().0;

        // Metadata link should exist with provider = "senscritique" (not "sc_trending_bd")
        let link_row = sqlx::query(
            "SELECT provider FROM external_metadata_links WHERE series_id = $1"
        )
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .expect("metadata link should exist for sc_ provider");
        let provider: String = link_row.get("provider");
        assert_eq!(provider, "senscritique", "sc_trending_bd should be normalized to senscritique");

        // Verify no link exists with original provider name
        let raw_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1 AND provider = 'sc_trending_bd'"
        )
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(raw_count, 0, "no link should exist with raw sc_trending_bd provider name");
    }

    // 5. Senscritique provider — metadata link created with correct provider
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn test_add_to_library_senscritique_provider(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool).await;
        let state = test_state(pool.clone());
        let req = make_request(library_id, "senscritique", "sc-direct-001", "SC Direct Series");

        let resp = add_to_library(State(state), Json(req)).await.unwrap().0;

        // Metadata link should exist with provider = "senscritique"
        let link_row = sqlx::query(
            "SELECT provider, external_id, status FROM external_metadata_links WHERE series_id = $1"
        )
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .expect("metadata link should exist for senscritique provider");
        let provider: String = link_row.get("provider");
        assert_eq!(provider, "senscritique");
        let ext_id: String = link_row.get("external_id");
        assert_eq!(ext_id, "sc-direct-001");
        let status: String = link_row.get("status");
        assert_eq!(status, "approved");

        // metadata_link_id should NOT equal series_id (a real link was created)
        assert_ne!(resp.metadata_link_id, resp.series_id, "metadata_link_id should be a real link UUID, not series_id");
    }
}

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
