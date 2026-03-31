use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use crate::metadata_providers::anilist;
use crate::metadata_providers::bedetheque;
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
    let limit = params.limit.unwrap_or(20).min(50);

    let supported = ["anilist", "bedetheque"];
    if !supported.contains(&provider) {
        return Err(ApiError::bad_request(format!(
            "trending not supported for provider '{provider}'. Supported: {}",
            supported.join(", ")
        )));
    }

    let cache_key = format!("trending:{provider}");

    // Check cache
    if let Some(cached) = get_cached(&state.pool, &cache_key).await {
        let filtered = filter_already_owned(&state.pool, provider, cached).await;
        return Ok(Json(filtered));
    }

    // Fetch from provider
    let candidates = match provider {
        "anilist" => anilist::fetch_trending(limit)
            .await
            .map_err(|e| ApiError::internal(format!("trending fetch failed: {e}")))?,
        "bedetheque" => bedetheque::fetch_indispensables(None, limit as usize)
            .await
            .map_err(|e| ApiError::internal(format!("indispensables fetch failed: {e}")))?,
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

    // Cache results
    set_cached(&state.pool, &cache_key, provider, "trending", &suggestions, 6).await;

    // Lazy cleanup
    cleanup_expired(&state.pool).await;

    // Filter out already-owned
    let filtered = filter_already_owned(&state.pool, provider, suggestions).await;
    Ok(Json(filtered))
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

    // 3. Create external_metadata_link (approved)
    let link_id: Uuid = sqlx::query_scalar(
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
    .bind(&req.provider)
    .bind(&req.external_id)
    .bind(&req.external_url)
    .bind(serde_json::json!({
        "genres": genres,
        "status": req.status,
    }))
    .bind(req.total_volumes)
    .fetch_one(&state.pool)
    .await?;

    tracing::info!(
        "[DISCOVERY] Added series '{}' to library {} from provider {}",
        req.title,
        req.library_id,
        req.provider
    );

    Ok(Json(AddToLibraryResponse {
        series_id,
        metadata_link_id: link_id,
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
