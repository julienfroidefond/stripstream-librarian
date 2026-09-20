use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use crate::metadata_providers::anilist;
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
    #[serde(default)]
    pub rating: Option<f64>,
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
pub struct HideRequest {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub cover_url: Option<String>,
}

#[derive(Serialize)]
pub struct HiddenItemDto {
    pub id: String,
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub hidden_at: String,
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
        "anilist",
        "senscritique",
        "senscritique_bd",
        "sc_trending_bd",
        "sc_trending_manga",
        "sc_best_bd",
        "sc_best_manga",
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
        "all" => "ALL_TIME",
        _ => "OUTOFMONTH",
    };
    let gql_sort = if provider.starts_with("sc_best") {
        "RATING"
    } else {
        "POPULARITY"
    };

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
            fetch_and_cache_trending(&state.pool, provider, &cache_key, gql_period, gql_sort)
                .await?
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
        "senscritique" | "senscritique_bd" | "sc_trending_bd" | "sc_trending_manga"
        | "sc_best_bd" | "sc_best_manga" => 100,
        _ => 50,
    };
    let candidates = match provider {
        "anilist" => anilist::fetch_trending(fetch_limit)
            .await
            .map_err(|e| ApiError::internal(format!("trending fetch failed: {e}")))?,
        "senscritique" => senscritique::fetch_top_mangas(fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("senscritique fetch failed: {e}")))?,
        "senscritique_bd" => senscritique::fetch_top_bd(fetch_limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("senscritique_bd fetch failed: {e}")))?,
        "sc_trending_bd" => senscritique::fetch_trending(
            "comicBook",
            gql_period,
            gql_sort,
            Some("BD franco-belge"),
            fetch_limit as usize,
        )
        .await
        .map_err(|e| ApiError::internal(format!("sc_trending_bd fetch failed: {e}")))?,
        "sc_trending_manga" => senscritique::fetch_trending(
            "comicBook",
            gql_period,
            gql_sort,
            Some("Manga"),
            fetch_limit as usize,
        )
        .await
        .map_err(|e| ApiError::internal(format!("sc_trending_manga fetch failed: {e}")))?,
        "sc_best_bd" => senscritique::fetch_trending(
            "comicBook",
            gql_period,
            "RATING",
            Some("BD franco-belge"),
            fetch_limit as usize,
        )
        .await
        .map_err(|e| ApiError::internal(format!("sc_best_bd fetch failed: {e}")))?,
        "sc_best_manga" => senscritique::fetch_trending(
            "comicBook",
            gql_period,
            "RATING",
            Some("Manga"),
            fetch_limit as usize,
        )
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
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let status = c
                .metadata_json
                .get("status")
                .and_then(|s| s.as_str())
                .map(String::from);
            let rating = c
                .metadata_json
                .get("rating")
                .and_then(|rating| rating.as_f64());
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
                rating,
            }
        })
        .collect();

    // Trending/best providers get 24h TTL; top/poll lists get infinite cache
    let ttl_hours = if provider.starts_with("sc_trending") || provider.starts_with("sc_best") {
        24
    } else {
        87600
    };
    set_cached(
        pool,
        cache_key,
        provider,
        "trending",
        &suggestions,
        ttl_hours,
    )
    .await;
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
    #[serde(default)]
    pub best_indexer: Option<String>,
    pub volumes_found: Vec<i32>,
    /// Local series matched by normalized name (unaccent + lowercase)
    #[serde(default)]
    pub local_series_id: Option<String>,
    #[serde(default)]
    pub local_series_name: Option<String>,
    /// Subset of volumes_found already present in the local series
    #[serde(default)]
    pub volumes_already_owned: Vec<i32>,
}

#[derive(Serialize)]
pub struct ProwlarrDiscoveryResponse {
    pub items: Vec<ProwlarrDiscoveryItem>,
    pub all_indexers: Vec<String>,
}

#[derive(Deserialize)]
pub struct ProwlarrDiscoveryQuery {
    pub nocache: Option<String>,
    /// "seeders" (default) or "date"
    pub sort: Option<String>,
    /// Filter results to a specific indexer name
    pub indexer: Option<String>,
    /// Restrict Prowlarr search to a single category ID (e.g. "7030")
    pub category: Option<String>,
}

/// GET /discovery/prowlarr — search Prowlarr indexers and group by series name
pub async fn prowlarr_discovery(
    State(state): State<AppState>,
    Query(params): Query<ProwlarrDiscoveryQuery>,
) -> Result<Json<ProwlarrDiscoveryResponse>, ApiError> {
    let sort_by_date = params.sort.as_deref() == Some("date");
    let indexer_filter = params.indexer.as_deref().filter(|s| !s.is_empty());
    let category_filter = params
        .category
        .as_deref()
        .and_then(|s| s.parse::<i32>().ok());

    // Cache key includes the category filter so single-category fetches don't pollute
    // the "all categories" cache and vice versa.
    let cache_key = match category_filter {
        Some(cat_id) => format!("discovery:prowlarr:cat:{cat_id}"),
        None => "discovery:prowlarr".to_string(),
    };
    let skip_cache = params.nocache.as_deref() == Some("true");

    // Helper: collect unique sorted indexers from a slice
    fn all_indexers_from(items: &[ProwlarrDiscoveryItem]) -> Vec<String> {
        let set: std::collections::BTreeSet<String> = items
            .iter()
            .flat_map(|i| i.indexers.iter().cloned())
            .collect();
        set.into_iter().collect()
    }

    // Sort a mutable slice by the requested mode
    fn apply_sort(items: &mut [ProwlarrDiscoveryItem], by_date: bool) {
        if by_date {
            items.sort_by(|a, b| {
                b.best_publish_date
                    .as_deref()
                    .unwrap_or("")
                    .cmp(a.best_publish_date.as_deref().unwrap_or(""))
            });
        } else {
            items.sort_by_key(|item| std::cmp::Reverse(item.best_seeders));
        }
    }

    // Check cache (unless nocache requested)
    if !skip_cache {
        if let Some(mut cached) =
            get_cached_raw::<Vec<ProwlarrDiscoveryItem>>(&state.pool, &cache_key).await
        {
            apply_sort(&mut cached, sort_by_date);
            let all_indexers = all_indexers_from(&cached);
            // Filter by indexer first to reduce the ownership-check workload
            let pre_filtered: Vec<ProwlarrDiscoveryItem> = if let Some(idx) = indexer_filter {
                cached
                    .into_iter()
                    .filter(|i| i.indexers.iter().any(|x| x == idx))
                    .collect()
            } else {
                cached
            };
            let annotated = annotate_local_matches(&state.pool, pre_filtered).await;
            let items: Vec<ProwlarrDiscoveryItem> = annotated;
            return Ok(Json(ProwlarrDiscoveryResponse {
                items,
                all_indexers,
            }));
        }
    }

    // Acquire fetch lock to prevent concurrent fetches from hammering indexers
    let _fetch_guard = state.prowlarr_fetch_lock.lock().await;

    // Re-check cache after acquiring lock — a concurrent request may have populated it
    if !skip_cache {
        if let Some(mut cached) =
            get_cached_raw::<Vec<ProwlarrDiscoveryItem>>(&state.pool, &cache_key).await
        {
            apply_sort(&mut cached, sort_by_date);
            let all_indexers = all_indexers_from(&cached);
            let pre_filtered: Vec<ProwlarrDiscoveryItem> = if let Some(idx) = indexer_filter {
                cached
                    .into_iter()
                    .filter(|i| i.indexers.iter().any(|x| x == idx))
                    .collect()
            } else {
                cached
            };
            let annotated = annotate_local_matches(&state.pool, pre_filtered).await;
            let items: Vec<ProwlarrDiscoveryItem> = annotated;
            return Ok(Json(ProwlarrDiscoveryResponse {
                items,
                all_indexers,
            }));
        }
    }

    // Load Prowlarr config
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'prowlarr'")
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("Prowlarr is not configured"))?;
    let value: serde_json::Value = row.get("value");
    let prowlarr_url = value
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let api_key = value
        .get("api_key")
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .to_string();
    let configured_categories: Vec<i32> = value
        .get("categories")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_i64().map(|n| n as i32))
                .collect()
        })
        .unwrap_or_else(|| vec![7030, 7020]);
    // When a specific category is requested, restrict the Prowlarr query to that one
    // so all 100 results per pass are focused on that category.
    let categories: Vec<i32> = match category_filter {
        Some(cat_id) => vec![cat_id],
        None => configured_categories,
    };

    if prowlarr_url.is_empty() || api_key.is_empty() {
        return Err(ApiError::bad_request(
            "Prowlarr URL and API key must be configured",
        ));
    }

    let client = reqwest::Client::builder()
        .user_agent("Stripstream-Librarian")
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| ApiError::internal(format!("HTTP client error: {e}")))?;

    // Prowlarr hard-caps live search at 100 results regardless of offset — pagination doesn't work.
    // Two passes to maximise coverage: default sort (seeders) + publishDate for recent releases.
    // GUID deduplication below ensures no double-counting.
    struct ProwlarrPass {
        sort_key: Option<&'static str>,
    }
    let passes: Vec<ProwlarrPass> = vec![
        ProwlarrPass { sort_key: None },
        ProwlarrPass {
            sort_key: Some("publishDate"),
        },
    ];

    let mut raw: Vec<serde_json::Value> = Vec::new();

    for pass in &passes {
        let mut params_vec: Vec<(&str, String)> = vec![
            ("query", String::new()),
            ("type", "search".to_string()),
            ("limit", "100".to_string()),
        ];
        for cat in &categories {
            params_vec.push(("categories", cat.to_string()));
        }
        if let Some(sort_key) = pass.sort_key {
            params_vec.push(("sortKey", sort_key.to_string()));
            params_vec.push(("sortDirection", "descending".to_string()));
        }

        let request = client
            .get(format!("{prowlarr_url}/api/v1/search"))
            .query(&params_vec)
            .header("X-Api-Key", &api_key)
            .build()
            .map_err(|e| ApiError::internal(format!("Prowlarr request build error: {e}")))?;
        tracing::info!("[DISCOVERY] Prowlarr request URL: {}", request.url());
        let resp = client.execute(request).await;

        match resp {
            Err(e) if e.is_connect() || e.is_timeout() => {
                return Err(ApiError::internal(format!("Prowlarr unreachable: {e}")));
            }
            Err(e) => {
                tracing::warn!(
                    "[DISCOVERY] Prowlarr request error sort={:?}: {e}",
                    pass.sort_key
                );
            }
            Ok(resp) if resp.status().is_success() => {
                let results: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
                let got = results.len();
                raw.extend(results);
                tracing::info!(
                    "[DISCOVERY] sort={:?} → {got} results (total raw: {})",
                    pass.sort_key,
                    raw.len()
                );
            }
            Ok(resp) => {
                tracing::warn!(
                    "[DISCOVERY] Prowlarr non-success status={} sort={:?}",
                    resp.status(),
                    pass.sort_key
                );
            }
        }

        // Delay between requests to avoid triggering indexer rate limits
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    if raw.is_empty() {
        return Ok(Json(ProwlarrDiscoveryResponse {
            items: vec![],
            all_indexers: vec![],
        }));
    }

    // One item per release — no aggregation by series.
    // Deduplicate by GUID to avoid counting the same release from multiple passes.
    let mut seen_guids = std::collections::HashSet::new();
    let mut items: Vec<ProwlarrDiscoveryItem> = Vec::new();

    for release in &raw {
        let guid = release
            .get("guid")
            .and_then(|g| g.as_str())
            .unwrap_or("")
            .to_string();
        if !guid.is_empty() && !seen_guids.insert(guid) {
            continue;
        }

        let title = release
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        if title.is_empty() {
            continue;
        }
        let seeders = release.get("seeders").and_then(|s| s.as_i64()).unwrap_or(0) as i32;
        let size = release.get("size").and_then(|s| s.as_i64()).unwrap_or(0);
        let download_url = release
            .get("downloadUrl")
            .and_then(|u| u.as_str())
            .map(String::from);
        let indexer = release
            .get("indexer")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        let publish_date = release
            .get("publishDate")
            .and_then(|d| d.as_str())
            .map(String::from);
        let info_url = release
            .get("infoUrl")
            .and_then(|u| u.as_str())
            .map(String::from);
        let cats: Vec<String> = release
            .get("categories")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        let name = v.get("name").and_then(|n| n.as_str())?;
                        let id = v.get("id").and_then(|i| i.as_i64())?;
                        Some(format!("{name} ({id})"))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Extract series name for local-library annotation only
        let series_name = parsers::extract_series_name_from_release(&title);
        let volumes = parsers::extract_volumes(&title);

        items.push(ProwlarrDiscoveryItem {
            series_name,
            release_count: 1,
            best_seeders: seeders,
            total_seeders: seeders,
            categories: cats,
            indexers: if indexer.is_empty() {
                vec![]
            } else {
                vec![indexer.clone()]
            },
            best_release_title: title,
            best_download_url: download_url,
            best_size: size,
            best_publish_date: publish_date,
            best_info_url: info_url,
            best_indexer: if indexer.is_empty() {
                None
            } else {
                Some(indexer)
            },
            volumes_found: volumes,
            local_series_id: None,
            local_series_name: None,
            volumes_already_owned: vec![],
        });
    }

    // Sort by seeders by default; apply_sort will re-order per-request
    items.sort_by_key(|item| std::cmp::Reverse(item.best_seeders));

    tracing::info!(
        "[DISCOVERY] Prowlarr: {} releases (deduplicated from {} raw)",
        items.len(),
        raw.len()
    );

    // Cache full result set (7 days)
    set_cached_raw(
        &state.pool,
        &cache_key,
        "prowlarr",
        "discovery",
        &items,
        168,
    )
    .await;

    apply_sort(&mut items, sort_by_date);
    let all_indexers = all_indexers_from(&items);

    // Filter by indexer then annotate with local library matches
    let pre_filtered: Vec<ProwlarrDiscoveryItem> = if let Some(idx) = indexer_filter {
        items
            .into_iter()
            .filter(|i| i.indexers.iter().any(|x| x == idx))
            .collect()
    } else {
        items
    };
    let annotated = annotate_local_matches(&state.pool, pre_filtered).await;
    Ok(Json(ProwlarrDiscoveryResponse {
        items: annotated,
        all_indexers,
    }))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod extract_tests {
    use parsers::extract_series_name_from_release;

    #[test]
    fn dot_t_volume() {
        // T31 → stop
        assert_eq!(
            extract_series_name_from_release(
                "Orcs.&.Gobelins.T31.Tren'gar.Peru.Sentenac.2025.FR.[CBZ]-NRC"
            ),
            "Orcs & Gobelins"
        );
        assert_eq!(
            extract_series_name_from_release(
                "Orcs.&.Gobelins.T32.Ogoor.Jarry.Scalisi.2025.FR.[CBZ]-NRC"
            ),
            "Orcs & Gobelins"
        );
        assert_eq!(
            extract_series_name_from_release(
                "Elric.T06.La.Sorciere.dormante.Blondel.Cano.2025.fr.[PDF].[CBZ]-notag"
            ),
            "Elric"
        );
    }

    #[test]
    fn dot_tome_split_volume() {
        // Tome/Vol standalone always a stop word
        assert_eq!(
            extract_series_name_from_release("Monstress.Tome.01.L'Éveil.LIU.FR.[PDF]-Notag"),
            "Monstress"
        );
        // Title with spaces inside brackets — dot heuristic still applies
        assert_eq!(
            extract_series_name_from_release(
                "One.Piece.Tome.[1 à 100].3.HS.Eichiro.Oda.FR.[CBZ]-GRP"
            ),
            "One Piece"
        );
    }

    #[test]
    fn dot_integrale_bracket() {
        // [INTEGRALE] and [COLLECTION] stop via bracket detection
        assert_eq!(
            extract_series_name_from_release("Gunnm.[INTEGRALE].FR.[CBZ]-PRiNTER-PapriKa"),
            "Gunnm"
        );
        assert_eq!(
            extract_series_name_from_release("Meteors.[INTEGRALE].FR.[PDF]-NOTAG"),
            "Meteors"
        );
        assert_eq!(
            extract_series_name_from_release("Hot.Cousine.Nils.[COLLECTION].FR.[CBR]-NOTAG"),
            "Hot Cousine Nils"
        );
        assert_eq!(
            extract_series_name_from_release("Planètes.[INTEGRALE].FR.[CBZ]-PapriKa"),
            "Planètes"
        );
        assert_eq!(
            extract_series_name_from_release(
                "Sherlock.Holmes.[COLLECTION].29.Albums.Par.Editeur.FR.[PDF]-NOTAG"
            ),
            "Sherlock Holmes"
        );
    }

    #[test]
    fn dot_year_stop() {
        assert_eq!(
            extract_series_name_from_release(
                "Neon.Genesis.Evangelion.1998.[INTEGRALE].FR.[CBZ]-MangaFR"
            ),
            "Neon Genesis Evangelion"
        );
        assert_eq!(
            extract_series_name_from_release("Naruto.2024.FR.[CBZ]-GRP"),
            "Naruto"
        );
    }

    #[test]
    fn dot_lang_stop() {
        assert_eq!(
            extract_series_name_from_release(
                "Akira.Edition.Originale.Katsuhiro.Otomo.FR.CBZ-Manga.Fr"
            ),
            "Akira Edition Originale Katsuhiro Otomo"
        );
        assert_eq!(
            extract_series_name_from_release("Star.Wars.Mega.Pack.Comics.FRENCH.[PDF]-Moorea81"),
            "Star Wars Mega Pack Comics"
        );
    }

    #[test]
    fn space_separated_tome() {
        assert_eq!(extract_series_name_from_release("Akira tome 1"), "Akira");
    }

    #[test]
    fn space_separated_bracket() {
        assert_eq!(
            extract_series_name_from_release("Dragon Ball [CBZ]"),
            "Dragon Ball"
        );
    }

    #[test]
    fn no_volume_marker_unchanged() {
        assert_eq!(extract_series_name_from_release("MySeries"), "MySeries");
    }
}

/// Annotate Prowlarr items with matching local series (2 batch queries).
/// Uses unaccent + lowercase matching to catch accent differences.
async fn annotate_local_matches(
    pool: &sqlx::PgPool,
    mut items: Vec<ProwlarrDiscoveryItem>,
) -> Vec<ProwlarrDiscoveryItem> {
    if items.is_empty() {
        return items;
    }

    let item_names: Vec<String> = items.iter().map(|i| i.series_name.clone()).collect();

    // Normalize both sides with unaccent so "Asterix" matches "Astérix" in DB
    let matched_rows = sqlx::query(
        "SELECT s.id::text, s.name, inputs.raw_name \
         FROM series s \
         JOIN (SELECT unnest($1::text[]) AS raw_name) AS inputs \
           ON norm_text(s.name) = norm_text(inputs.raw_name)",
    )
    .bind(&item_names)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if matched_rows.is_empty() {
        return items;
    }

    // Build raw_item_name → (series_id, series_name) map
    let match_map: std::collections::HashMap<String, (String, String)> = matched_rows
        .iter()
        .map(|r| {
            let raw: String = r.get("raw_name");
            let id: String = r.get("id");
            let name: String = r.get("name");
            (raw, (id, name))
        })
        .collect();

    let matched_ids: Vec<String> = match_map.values().map(|(id, _)| id.clone()).collect();

    // Query 2: get all regular volume numbers for matched series
    let volume_rows = sqlx::query(
        "SELECT series_id::text, volume_number \
         FROM books \
         WHERE series_id::text = ANY($1) AND volume_type = 'regular' AND volume_number IS NOT NULL",
    )
    .bind(&matched_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // series_id → Vec<volume_number>
    let mut owned_volumes: std::collections::HashMap<String, Vec<i32>> =
        std::collections::HashMap::new();
    for row in &volume_rows {
        let sid: String = row.get("series_id");
        let vol: i32 = row.get("volume_number");
        owned_volumes.entry(sid).or_default().push(vol);
    }

    // Annotate items — look up by the original item name (matched in SQL via unaccent)
    for item in &mut items {
        if let Some((sid, sname)) = match_map.get(&item.series_name) {
            let owned = owned_volumes.get(sid).cloned().unwrap_or_default();
            item.volumes_already_owned = item
                .volumes_found
                .iter()
                .filter(|v| owned.contains(v))
                .copied()
                .collect();
            item.local_series_id = Some(sid.clone());
            item.local_series_name = Some(sname.clone());
        }
    }

    items
}

// ─── Generic cache helpers for typed data ───────────────────────────────────

async fn get_cached_raw<T: serde::de::DeserializeOwned>(
    pool: &sqlx::PgPool,
    cache_key: &str,
) -> Option<T> {
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
    use crate::series::helpers::{create_series_with_metadata, CreateSeriesParams};

    // Normalize provider: sc_trending_bd, sc_best_manga, etc. → senscritique
    let metadata_provider = if req.provider.starts_with("sc_") {
        "senscritique".to_string()
    } else {
        req.provider.clone()
    };

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

    let result = create_series_with_metadata(
        &state,
        CreateSeriesParams {
            library_id: req.library_id,
            name: req.title.clone(),
            provider: Some(metadata_provider.clone()),
            external_id: Some(req.external_id.clone()),
            external_url: req.external_url.clone(),
            confidence: Some(1.0),
            total_volumes: req.total_volumes,
            metadata_json: Some(metadata_json),
        },
    )
    .await?;

    tracing::info!(
        "[DISCOVERY] Added series '{}' to library {} from provider {}{}",
        req.title,
        req.library_id,
        req.provider,
        if result.metadata_link_id.is_some() {
            " (metadata link created)"
        } else {
            ""
        }
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
    let owned_by_name: Vec<String> =
        sqlx::query_scalar("SELECT LOWER(name) FROM series WHERE LOWER(name) = ANY($1)")
            .bind(&titles)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    // 3. Filter by hidden items
    let hidden_ids: Vec<String> =
        sqlx::query_scalar("SELECT external_id FROM discovery_hidden WHERE external_id = ANY($1)")
            .bind(&external_ids)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    suggestions
        .into_iter()
        .filter(|s| {
            !owned_by_link.contains(&s.external_id)
                && !owned_by_name.contains(&s.title.to_lowercase())
                && !hidden_ids.contains(&s.external_id)
        })
        .collect()
}

// ─── Hide / unhide ──────────────────────────────────────────────────────

/// Hide a discovery suggestion so it no longer appears in results.
pub async fn hide_suggestion(
    State(state): State<AppState>,
    Json(body): Json<HideRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query(
        "INSERT INTO discovery_hidden (provider, external_id, title, cover_url) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (provider, external_id) DO NOTHING",
    )
    .bind(&body.provider)
    .bind(&body.external_id)
    .bind(&body.title)
    .bind(&body.cover_url)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({"hidden": true})))
}

/// Unhide a previously hidden discovery suggestion.
pub async fn unhide_suggestion(
    State(state): State<AppState>,
    Json(body): Json<HideRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query("DELETE FROM discovery_hidden WHERE provider = $1 AND external_id = $2")
        .bind(&body.provider)
        .bind(&body.external_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(serde_json::json!({"hidden": false})))
}

/// List all hidden discovery suggestions.
pub async fn list_hidden(
    State(state): State<AppState>,
) -> Result<Json<Vec<HiddenItemDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, provider, external_id, title, cover_url, hidden_at \
         FROM discovery_hidden ORDER BY hidden_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<HiddenItemDto> = rows
        .iter()
        .map(|r| HiddenItemDto {
            id: r.get::<Uuid, _>("id").to_string(),
            provider: r.get("provider"),
            external_id: r.get("external_id"),
            title: r.get("title"),
            cover_url: r.get("cover_url"),
            hidden_at: r
                .get::<chrono::DateTime<chrono::Utc>, _>("hidden_at")
                .to_rfc3339(),
        })
        .collect();

    Ok(Json(items))
}
