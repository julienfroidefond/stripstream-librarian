use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ─── AniList API client ───────────────────────────────────────────────────────

const ANILIST_API: &str = "https://graphql.anilist.co";

pub(crate) async fn anilist_graphql(
    token: &str,
    query: &str,
    variables: Value,
) -> Result<Value, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| ApiError::internal(format!("HTTP client error: {e}")))?;

    let body = serde_json::json!({ "query": query, "variables": variables });

    let resp = client
        .post(ANILIST_API)
        .bearer_auth(token)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("AniList request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::internal(format!("AniList returned {status}: {text}")));
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse AniList response: {e}")))?;

    if let Some(errors) = data.get("errors") {
        let msg = errors[0]["message"].as_str().unwrap_or("Unknown AniList error");
        return Err(ApiError::internal(format!("AniList API error: {msg}")));
    }

    Ok(data["data"].clone())
}

/// Load AniList settings from DB: (access_token, anilist_user_id, local_user_id)
pub(crate) async fn load_anilist_settings(pool: &sqlx::PgPool) -> Result<(String, Option<i64>, Option<Uuid>), ApiError> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'anilist'")
        .fetch_optional(pool)
        .await?;

    let value: Value = row
        .ok_or_else(|| ApiError::bad_request("AniList not configured (missing settings)"))?
        .get("value");

    let token = value["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("AniList access token not configured"))?
        .to_string();

    let user_id = value["user_id"].as_i64();

    let local_user_id = value["local_user_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok());

    Ok((token, user_id, local_user_id))
}

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct AnilistStatusResponse {
    pub connected: bool,
    pub user_id: i64,
    pub username: String,
    pub site_url: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AnilistMediaResult {
    pub id: i32,
    pub title_romaji: Option<String>,
    pub title_english: Option<String>,
    pub title_native: Option<String>,
    pub site_url: String,
    pub status: Option<String>,
    pub volumes: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct AnilistSeriesLinkResponse {
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub series_name: String,
    pub anilist_id: i32,
    pub anilist_title: Option<String>,
    pub anilist_url: Option<String>,
    pub status: String,
    #[schema(value_type = String)]
    pub linked_at: DateTime<Utc>,
    #[schema(value_type = Option<String>)]
    pub synced_at: Option<DateTime<Utc>>,
}

// Sync types and handlers are in super::anilist_sync
pub use super::anilist_sync::{preview_sync, sync_to_anilist, pull_from_anilist};

#[derive(Deserialize, ToSchema)]
pub struct AnilistSearchRequest {
    pub query: String,
}

#[derive(Deserialize, ToSchema)]
pub struct AnilistLinkRequest {
    pub anilist_id: i32,
    /// Override display title (optional)
    pub title: Option<String>,
    /// Override URL (optional)
    pub url: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct AnilistLibraryToggleRequest {
    pub enabled: bool,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// Test AniList connection and return viewer info
#[utoipa::path(
    get,
    path = "/anilist/status",
    tag = "anilist",
    responses(
        (status = 200, body = AnilistStatusResponse),
        (status = 400, description = "AniList not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_status(
    State(state): State<AppState>,
) -> Result<Json<AnilistStatusResponse>, ApiError> {
    let (token, _, _) = load_anilist_settings(&state.pool).await?;

    let gql = r#"
        query Viewer {
            Viewer {
                id
                name
                siteUrl
            }
        }
    "#;

    let data = anilist_graphql(&token, gql, serde_json::json!({})).await?;

    let viewer = &data["Viewer"];
    Ok(Json(AnilistStatusResponse {
        connected: true,
        user_id: viewer["id"].as_i64().unwrap_or(0),
        username: viewer["name"].as_str().unwrap_or("").to_string(),
        site_url: viewer["siteUrl"].as_str().unwrap_or("").to_string(),
    }))
}

/// Search AniList manga by title
#[utoipa::path(
    post,
    path = "/anilist/search",
    tag = "anilist",
    request_body = AnilistSearchRequest,
    responses(
        (status = 200, body = Vec<AnilistMediaResult>),
        (status = 400, description = "AniList not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn search_manga(
    State(state): State<AppState>,
    Json(body): Json<AnilistSearchRequest>,
) -> Result<Json<Vec<AnilistMediaResult>>, ApiError> {
    let (token, _, _) = load_anilist_settings(&state.pool).await?;

    let gql = r#"
        query SearchManga($search: String) {
            Page(perPage: 10) {
                media(search: $search, type: MANGA) {
                    id
                    title { romaji english native }
                    siteUrl
                    status
                    volumes
                }
            }
        }
    "#;

    let data = anilist_graphql(
        &token,
        gql,
        serde_json::json!({ "search": body.query }),
    )
    .await?;

    let media = data["Page"]["media"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let results: Vec<AnilistMediaResult> = media
        .iter()
        .map(|m| AnilistMediaResult {
            id: m["id"].as_i64().unwrap_or(0) as i32,
            title_romaji: m["title"]["romaji"].as_str().map(String::from),
            title_english: m["title"]["english"].as_str().map(String::from),
            title_native: m["title"]["native"].as_str().map(String::from),
            site_url: m["siteUrl"].as_str().unwrap_or("").to_string(),
            status: m["status"].as_str().map(String::from),
            volumes: m["volumes"].as_i64().map(|v| v as i32),
        })
        .collect();

    Ok(Json(results))
}

/// Get AniList link for a specific series (deprecated: use GET /series/{series_id}/anilist)
#[deprecated]
#[utoipa::path(
    get,
    path = "/anilist/series/{library_id}/{series_id}",
    tag = "anilist (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = AnilistSeriesLinkResponse),
        (status = 404, description = "No AniList link for this series"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_link(
    State(state): State<AppState>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<AnilistSeriesLinkResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT asl.library_id, s.name AS series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url, asl.status, asl.linked_at, asl.synced_at
         FROM anilist_series_links asl
         JOIN series s ON s.id = asl.series_id
         WHERE asl.library_id = $1 AND asl.series_id = $2",
    )
    .bind(library_id)
    .bind(series_id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("no AniList link for this series"))?;

    Ok(Json(AnilistSeriesLinkResponse {
        library_id: row.get("library_id"),
        series_name: row.get("series_name"),
        anilist_id: row.get("anilist_id"),
        anilist_title: row.get("anilist_title"),
        anilist_url: row.get("anilist_url"),
        status: row.get("status"),
        linked_at: row.get("linked_at"),
        synced_at: row.get("synced_at"),
    }))
}

/// Link a series to an AniList media ID (deprecated: use POST /series/{series_id}/anilist/link)
#[deprecated]
#[utoipa::path(
    post,
    path = "/anilist/series/{library_id}/{series_id}/link",
    tag = "anilist (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    request_body = AnilistLinkRequest,
    responses(
        (status = 200, body = AnilistSeriesLinkResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn link_series(
    State(state): State<AppState>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<AnilistLinkRequest>,
) -> Result<Json<AnilistSeriesLinkResponse>, ApiError> {
    // Try to fetch title/url from AniList if not provided
    let (anilist_title, anilist_url) = if body.title.is_some() && body.url.is_some() {
        (body.title, body.url)
    } else {
        // Fetch from AniList
        match load_anilist_settings(&state.pool).await {
            Ok((token, _, _)) => {
                let gql = r#"
                    query GetMedia($id: Int) {
                        Media(id: $id, type: MANGA) {
                            title { romaji english }
                            siteUrl
                        }
                    }
                "#;
                match anilist_graphql(&token, gql, serde_json::json!({ "id": body.anilist_id })).await {
                    Ok(data) => {
                        let title = data["Media"]["title"]["english"]
                            .as_str()
                            .or_else(|| data["Media"]["title"]["romaji"].as_str())
                            .map(String::from);
                        let url = data["Media"]["siteUrl"].as_str().map(String::from);
                        (title, url)
                    }
                    Err(_) => (body.title, body.url),
                }
            }
            Err(_) => (body.title, body.url),
        }
    };

    let row = sqlx::query(
        r#"
        INSERT INTO anilist_series_links (library_id, series_id, provider, anilist_id, anilist_title, anilist_url, status, linked_at)
        VALUES ($1, $2, 'anilist', $3, $4, $5, 'linked', NOW())
        ON CONFLICT (series_id, provider) DO UPDATE
          SET anilist_id = EXCLUDED.anilist_id,
              anilist_title = EXCLUDED.anilist_title,
              anilist_url = EXCLUDED.anilist_url,
              status = 'linked',
              linked_at = NOW(),
              synced_at = NULL
        RETURNING library_id, series_id, anilist_id, anilist_title, anilist_url, status, linked_at, synced_at
        "#,
    )
    .bind(library_id)
    .bind(series_id)
    .bind(body.anilist_id)
    .bind(&anilist_title)
    .bind(&anilist_url)
    .fetch_one(&state.pool)
    .await?;

    // Fetch series name for the response
    let series_name: String = sqlx::query_scalar("SELECT name FROM series WHERE id = $1")
        .bind(series_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(Json(AnilistSeriesLinkResponse {
        library_id: row.get("library_id"),
        series_name,
        anilist_id: row.get("anilist_id"),
        anilist_title: row.get("anilist_title"),
        anilist_url: row.get("anilist_url"),
        status: row.get("status"),
        linked_at: row.get("linked_at"),
        synced_at: row.get("synced_at"),
    }))
}

/// Remove the AniList link for a series (deprecated: use DELETE /series/{series_id}/anilist/unlink)
#[deprecated]
#[utoipa::path(
    delete,
    path = "/anilist/series/{library_id}/{series_id}/unlink",
    tag = "anilist (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Unlinked"),
        (status = 404, description = "Link not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn unlink_series(
    State(state): State<AppState>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<crate::responses::UnlinkedResponse>, ApiError> {
    let result = sqlx::query(
        "DELETE FROM anilist_series_links WHERE library_id = $1 AND series_id = $2",
    )
    .bind(library_id)
    .bind(series_id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("AniList link not found"));
    }

    Ok(Json(crate::responses::UnlinkedResponse::new()))
}

// ─── Direct series-by-ID AniList endpoints ──────────────────────────────────

/// Get AniList link for a series by its UUID
#[utoipa::path(
    get,
    path = "/series/{series_id}/anilist",
    tag = "anilist",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = AnilistSeriesLinkResponse),
        (status = 404, description = "No AniList link for this series"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn get_series_link_by_id(
    state: State<AppState>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<AnilistSeriesLinkResponse>, ApiError> {
    let library_id = crate::series::resolve_library_id(&state.pool, series_id).await?;
    get_series_link(state, Path((library_id, series_id))).await
}

/// Link a series to AniList by its UUID
#[utoipa::path(
    post,
    path = "/series/{series_id}/anilist/link",
    tag = "anilist",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    request_body = AnilistLinkRequest,
    responses(
        (status = 200, body = AnilistSeriesLinkResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn link_series_by_id(
    state: State<AppState>,
    Path(series_id): Path<Uuid>,
    body: Json<AnilistLinkRequest>,
) -> Result<Json<AnilistSeriesLinkResponse>, ApiError> {
    let library_id = crate::series::resolve_library_id(&state.pool, series_id).await?;
    link_series(state, Path((library_id, series_id)), body).await
}

/// Remove AniList link for a series by its UUID
#[utoipa::path(
    delete,
    path = "/series/{series_id}/anilist/unlink",
    tag = "anilist",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, description = "Unlinked"),
        (status = 404, description = "Link not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn unlink_series_by_id(
    state: State<AppState>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<crate::responses::UnlinkedResponse>, ApiError> {
    let library_id = crate::series::resolve_library_id(&state.pool, series_id).await?;
    unlink_series(state, Path((library_id, series_id))).await
}

/// Toggle AniList sync for a library
#[utoipa::path(
    patch,
    path = "/anilist/libraries/{id}",
    tag = "anilist",
    params(("id" = String, Path, description = "Library UUID")),
    request_body = AnilistLibraryToggleRequest,
    responses(
        (status = 200, description = "Updated"),
        (status = 404, description = "Library not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn toggle_library(
    State(state): State<AppState>,
    Path(library_id): Path<Uuid>,
    Json(body): Json<AnilistLibraryToggleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let provider: Option<&str> = if body.enabled { Some("anilist") } else { None };
    let result = sqlx::query("UPDATE libraries SET reading_status_provider = $2 WHERE id = $1")
        .bind(library_id)
        .bind(provider)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("library not found"));
    }

    Ok(Json(serde_json::json!({ "library_id": library_id, "reading_status_provider": provider })))
}

/// List series from AniList-enabled libraries that are not yet linked
#[utoipa::path(
    get,
    path = "/anilist/unlinked",
    tag = "anilist",
    responses(
        (status = 200, description = "List of unlinked series"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_unlinked(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            l.id AS library_id,
            l.name AS library_name,
            COALESCE(s.name, 'unclassified') AS series_name
        FROM books b
        JOIN libraries l ON l.id = b.library_id
        LEFT JOIN series s ON s.id = b.series_id
        LEFT JOIN anilist_series_links asl
            ON asl.series_id = b.series_id
        WHERE l.reading_status_provider = 'anilist'
          AND asl.series_id IS NULL
          AND b.series_id IS NOT NULL
        GROUP BY l.id, l.name, COALESCE(s.name, 'unclassified')
        ORDER BY l.name, series_name
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let library_id: Uuid = row.get("library_id");
            serde_json::json!({
                "library_id": library_id,
                "library_name": row.get::<String, _>("library_name"),
                "series_name": row.get::<String, _>("series_name"),
            })
        })
        .collect();

    Ok(Json(items))
}

// preview_sync, sync_to_anilist, pull_from_anilist are now in super::anilist_sync

// preview_sync, sync_to_anilist, pull_from_anilist are now in super::anilist_sync

/// List all AniList series links
#[utoipa::path(
    get,
    path = "/anilist/links",
    tag = "anilist",
    responses(
        (status = 200, body = Vec<AnilistSeriesLinkResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_links(
    State(state): State<AppState>,
) -> Result<Json<Vec<AnilistSeriesLinkResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT asl.library_id, s.name AS series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url, asl.status, asl.linked_at, asl.synced_at
         FROM anilist_series_links asl
         JOIN series s ON s.id = asl.series_id
         ORDER BY asl.linked_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    let links: Vec<AnilistSeriesLinkResponse> = rows
        .iter()
        .map(|row| AnilistSeriesLinkResponse {
            library_id: row.get("library_id"),
            series_name: row.get("series_name"),
            anilist_id: row.get("anilist_id"),
            anilist_title: row.get("anilist_title"),
            anilist_url: row.get("anilist_url"),
            status: row.get("status"),
            linked_at: row.get("linked_at"),
            synced_at: row.get("synced_at"),
        })
        .collect();

    Ok(Json(links))
}
