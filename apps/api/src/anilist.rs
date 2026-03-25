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

#[derive(Serialize, ToSchema)]
pub struct AnilistSyncPreviewItem {
    pub series_name: String,
    pub anilist_id: i32,
    pub anilist_title: Option<String>,
    pub anilist_url: Option<String>,
    /// Status that would be sent to AniList: PLANNING | CURRENT | COMPLETED
    pub status: String,
    pub progress_volumes: i32,
    pub books_read: i64,
    pub book_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct AnilistSyncItem {
    pub series_name: String,
    pub anilist_title: Option<String>,
    pub anilist_url: Option<String>,
    /// Status sent to AniList: PLANNING | CURRENT | COMPLETED
    pub status: String,
    pub progress_volumes: i32,
}

#[derive(Serialize, ToSchema)]
pub struct AnilistSyncReport {
    pub synced: i32,
    pub skipped: i32,
    pub errors: Vec<String>,
    pub items: Vec<AnilistSyncItem>,
}

#[derive(Serialize, ToSchema)]
pub struct AnilistPullItem {
    pub series_name: String,
    pub anilist_title: Option<String>,
    pub anilist_url: Option<String>,
    /// Status received from AniList: COMPLETED | CURRENT | PLANNING | etc.
    pub anilist_status: String,
    pub books_updated: i32,
}

#[derive(Serialize, ToSchema)]
pub struct AnilistPullReport {
    pub updated: i32,
    pub skipped: i32,
    pub errors: Vec<String>,
    pub items: Vec<AnilistPullItem>,
}

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

/// Get AniList link for a specific series
#[utoipa::path(
    get,
    path = "/anilist/series/{library_id}/{series_name}",
    tag = "anilist",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_name" = String, Path, description = "Series name"),
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
    Path((library_id, series_name)): Path<(Uuid, String)>,
) -> Result<Json<AnilistSeriesLinkResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT library_id, series_name, anilist_id, anilist_title, anilist_url, status, linked_at, synced_at
         FROM anilist_series_links
         WHERE library_id = $1 AND series_name = $2",
    )
    .bind(library_id)
    .bind(&series_name)
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

/// Link a series to an AniList media ID
#[utoipa::path(
    post,
    path = "/anilist/series/{library_id}/{series_name}/link",
    tag = "anilist",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_name" = String, Path, description = "Series name"),
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
    Path((library_id, series_name)): Path<(Uuid, String)>,
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
        INSERT INTO anilist_series_links (library_id, series_name, provider, anilist_id, anilist_title, anilist_url, status, linked_at)
        VALUES ($1, $2, 'anilist', $3, $4, $5, 'linked', NOW())
        ON CONFLICT (library_id, series_name, provider) DO UPDATE
          SET anilist_id = EXCLUDED.anilist_id,
              anilist_title = EXCLUDED.anilist_title,
              anilist_url = EXCLUDED.anilist_url,
              status = 'linked',
              linked_at = NOW(),
              synced_at = NULL
        RETURNING library_id, series_name, anilist_id, anilist_title, anilist_url, status, linked_at, synced_at
        "#,
    )
    .bind(library_id)
    .bind(&series_name)
    .bind(body.anilist_id)
    .bind(&anilist_title)
    .bind(&anilist_url)
    .fetch_one(&state.pool)
    .await?;

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

/// Remove the AniList link for a series
#[utoipa::path(
    delete,
    path = "/anilist/series/{library_id}/{series_name}/unlink",
    tag = "anilist",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_name" = String, Path, description = "Series name"),
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
    Path((library_id, series_name)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query(
        "DELETE FROM anilist_series_links WHERE library_id = $1 AND series_name = $2",
    )
    .bind(library_id)
    .bind(&series_name)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("AniList link not found"));
    }

    Ok(Json(serde_json::json!({"unlinked": true})))
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
            COALESCE(NULLIF(b.series, ''), 'unclassified') AS series_name
        FROM books b
        JOIN libraries l ON l.id = b.library_id
        LEFT JOIN anilist_series_links asl
            ON asl.library_id = b.library_id
            AND asl.series_name = COALESCE(NULLIF(b.series, ''), 'unclassified')
        WHERE l.reading_status_provider = 'anilist'
          AND asl.library_id IS NULL
        GROUP BY l.id, l.name, COALESCE(NULLIF(b.series, ''), 'unclassified')
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

/// Preview what would be synced to AniList (dry-run, no writes)
#[utoipa::path(
    get,
    path = "/anilist/sync/preview",
    tag = "anilist",
    responses(
        (status = 200, body = Vec<AnilistSyncPreviewItem>),
        (status = 400, description = "AniList not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn preview_sync(
    State(state): State<AppState>,
) -> Result<Json<Vec<AnilistSyncPreviewItem>>, ApiError> {
    let (_, _, local_user_id) = load_anilist_settings(&state.pool).await?;
    let local_user_id = local_user_id
        .ok_or_else(|| ApiError::bad_request("AniList local user not configured — please select a user in settings"))?;

    let links = sqlx::query(
        r#"
        SELECT asl.library_id, asl.series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url
        FROM anilist_series_links asl
        JOIN libraries l ON l.id = asl.library_id
        WHERE l.reading_status_provider = 'anilist'
        ORDER BY l.name, asl.series_name
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut items: Vec<AnilistSyncPreviewItem> = Vec::new();

    for link in &links {
        let library_id: Uuid = link.get("library_id");
        let series_name: String = link.get("series_name");
        let anilist_id: i32 = link.get("anilist_id");
        let anilist_title: Option<String> = link.get("anilist_title");
        let anilist_url: Option<String> = link.get("anilist_url");

        let stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read,
                (SELECT sm.total_volumes FROM series_metadata sm WHERE sm.library_id = $1 AND sm.name = $2 LIMIT 1) as total_volumes
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = $3
            WHERE b.library_id = $1 AND COALESCE(NULLIF(b.series, ''), 'unclassified') = $2
            "#,
        )
        .bind(library_id)
        .bind(&series_name)
        .bind(local_user_id)
        .fetch_one(&state.pool)
        .await;

        let (book_count, books_read, total_volumes) = match stats {
            Ok(row) => {
                let bc: i64 = row.get("book_count");
                let br: i64 = row.get("books_read");
                let tv: Option<i32> = row.get("total_volumes");
                (bc, br, tv)
            }
            Err(_) => continue,
        };

        if book_count == 0 {
            continue;
        }

        let (status, progress_volumes) = if books_read > 0 && total_volumes.is_some_and(|tv| books_read >= tv as i64) {
            ("COMPLETED".to_string(), books_read as i32)
        } else if books_read > 0 {
            ("CURRENT".to_string(), books_read as i32)
        } else {
            ("PLANNING".to_string(), 0i32)
        };

        items.push(AnilistSyncPreviewItem {
            series_name,
            anilist_id,
            anilist_title,
            anilist_url,
            status,
            progress_volumes,
            books_read,
            book_count,
        });
    }

    Ok(Json(items))
}

/// Sync local reading progress to AniList for all enabled libraries
#[utoipa::path(
    post,
    path = "/anilist/sync",
    tag = "anilist",
    responses(
        (status = 200, body = AnilistSyncReport),
        (status = 400, description = "AniList not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn sync_to_anilist(
    State(state): State<AppState>,
) -> Result<Json<AnilistSyncReport>, ApiError> {
    let (token, _, local_user_id) = load_anilist_settings(&state.pool).await?;
    let local_user_id = local_user_id
        .ok_or_else(|| ApiError::bad_request("AniList local user not configured — please select a user in settings"))?;

    // Get all series that have AniList links in enabled libraries
    let links = sqlx::query(
        r#"
        SELECT asl.library_id, asl.series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url
        FROM anilist_series_links asl
        JOIN libraries l ON l.id = asl.library_id
        WHERE l.reading_status_provider = 'anilist'
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut synced = 0i32;
    let mut skipped = 0i32;
    let mut errors: Vec<String> = Vec::new();
    let mut items: Vec<AnilistSyncItem> = Vec::new();

    let gql_update = r#"
        mutation SaveEntry($mediaId: Int, $status: MediaListStatus, $progressVolumes: Int) {
            SaveMediaListEntry(mediaId: $mediaId, status: $status, progressVolumes: $progressVolumes) {
                id
                status
                progressVolumes
            }
        }
    "#;

    for link in &links {
        let library_id: Uuid = link.get("library_id");
        let series_name: String = link.get("series_name");
        let anilist_id: i32 = link.get("anilist_id");
        let anilist_title: Option<String> = link.get("anilist_title");
        let anilist_url: Option<String> = link.get("anilist_url");

        // Get reading progress + total_volumes from series metadata
        let stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read,
                (SELECT sm.total_volumes FROM series_metadata sm WHERE sm.library_id = $1 AND sm.name = $2 LIMIT 1) as total_volumes
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = $3
            WHERE b.library_id = $1 AND COALESCE(NULLIF(b.series, ''), 'unclassified') = $2
            "#,
        )
        .bind(library_id)
        .bind(&series_name)
        .bind(local_user_id)
        .fetch_one(&state.pool)
        .await;

        let (book_count, books_read, total_volumes) = match stats {
            Ok(row) => {
                let bc: i64 = row.get("book_count");
                let br: i64 = row.get("books_read");
                let tv: Option<i32> = row.get("total_volumes");
                (bc, br, tv)
            }
            Err(e) => {
                errors.push(format!("{series_name}: DB error: {e}"));
                continue;
            }
        };

        // COMPLETED only if books_read reaches the known total_volumes
        // — never auto-complete based solely on owned books
        let (status, progress_volumes) = if book_count == 0 {
            skipped += 1;
            continue;
        } else if books_read > 0 && total_volumes.is_some_and(|tv| books_read >= tv as i64) {
            ("COMPLETED", books_read as i32)
        } else if books_read > 0 {
            ("CURRENT", books_read as i32)
        } else {
            ("PLANNING", 0i32)
        };

        let vars = serde_json::json!({
            "mediaId": anilist_id,
            "status": status,
            "progressVolumes": progress_volumes,
        });

        match anilist_graphql(&token, gql_update, vars).await {
            Ok(_) => {
                // Update synced_at
                let _ = sqlx::query(
                    "UPDATE anilist_series_links SET status = 'synced', synced_at = NOW() WHERE library_id = $1 AND series_name = $2",
                )
                .bind(library_id)
                .bind(&series_name)
                .execute(&state.pool)
                .await;
                items.push(AnilistSyncItem {
                    series_name: series_name.clone(),
                    anilist_title,
                    anilist_url,
                    status: status.to_string(),
                    progress_volumes,
                });
                synced += 1;
            }
            Err(e) => {
                let _ = sqlx::query(
                    "UPDATE anilist_series_links SET status = 'error' WHERE library_id = $1 AND series_name = $2",
                )
                .bind(library_id)
                .bind(&series_name)
                .execute(&state.pool)
                .await;
                errors.push(format!("{series_name}: {}", e.message));
            }
        }
    }

    Ok(Json(AnilistSyncReport { synced, skipped, errors, items }))
}

/// Pull reading list from AniList and update local reading progress
#[utoipa::path(
    post,
    path = "/anilist/pull",
    tag = "anilist",
    responses(
        (status = 200, body = AnilistPullReport),
        (status = 400, description = "AniList not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn pull_from_anilist(
    State(state): State<AppState>,
) -> Result<Json<AnilistPullReport>, ApiError> {
    let (token, user_id, local_user_id) = load_anilist_settings(&state.pool).await?;
    let user_id = user_id
        .ok_or_else(|| ApiError::bad_request("AniList user_id not configured — please test the connection in settings"))?;
    let local_user_id = local_user_id
        .ok_or_else(|| ApiError::bad_request("AniList local user not configured — please select a user in settings"))?;

    let gql = r#"
        query GetUserMangaList($userId: Int) {
            MediaListCollection(userId: $userId, type: MANGA) {
                lists {
                    entries {
                        media { id siteUrl }
                        status
                        progressVolumes
                    }
                }
            }
        }
    "#;

    let data = anilist_graphql(&token, gql, serde_json::json!({ "userId": user_id })).await?;

    let lists = data["MediaListCollection"]["lists"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    // Build flat list of (anilist_id, status, progressVolumes)
    let mut entries: Vec<(i32, String, i32)> = Vec::new();
    for list in &lists {
        if let Some(list_entries) = list["entries"].as_array() {
            for entry in list_entries {
                let media_id = entry["media"]["id"].as_i64().unwrap_or(0) as i32;
                let status = entry["status"].as_str().unwrap_or("").to_string();
                let progress = entry["progressVolumes"].as_i64().unwrap_or(0) as i32;
                entries.push((media_id, status, progress));
            }
        }
    }

    // Find local series linked to these anilist IDs (in enabled libraries)
    let link_rows = sqlx::query(
        r#"
        SELECT asl.library_id, asl.series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url
        FROM anilist_series_links asl
        JOIN libraries l ON l.id = asl.library_id
        WHERE l.reading_status_provider = 'anilist'
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    // Build map: anilist_id → (library_id, series_name, anilist_title, anilist_url)
    let mut link_map: std::collections::HashMap<i32, (Uuid, String, Option<String>, Option<String>)> =
        std::collections::HashMap::new();
    for row in &link_rows {
        let aid: i32 = row.get("anilist_id");
        let lib: Uuid = row.get("library_id");
        let name: String = row.get("series_name");
        let title: Option<String> = row.get("anilist_title");
        let url: Option<String> = row.get("anilist_url");
        link_map.insert(aid, (lib, name, title, url));
    }

    let mut updated = 0i32;
    let mut skipped = 0i32;
    let mut errors: Vec<String> = Vec::new();
    let mut items: Vec<AnilistPullItem> = Vec::new();

    for (anilist_id, anilist_status, progress_volumes) in &entries {
        let Some((library_id, series_name, anilist_title, anilist_url)) = link_map.get(anilist_id) else {
            skipped += 1;
            continue;
        };

        // Map AniList status → local reading status
        let local_status = match anilist_status.as_str() {
            "COMPLETED" => "read",
            "CURRENT" | "REPEATING" => "reading",
            "PLANNING" | "PAUSED" | "DROPPED" => "unread",
            _ => {
                skipped += 1;
                continue;
            }
        };

        // Get all book IDs for this series, ordered by volume
        let book_rows = sqlx::query(
            "SELECT id, volume FROM books WHERE library_id = $1 AND COALESCE(NULLIF(series, ''), 'unclassified') = $2 ORDER BY volume NULLS LAST",
        )
        .bind(library_id)
        .bind(series_name)
        .fetch_all(&state.pool)
        .await;

        let book_rows = match book_rows {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("{series_name}: {e}"));
                continue;
            }
        };

        if book_rows.is_empty() {
            skipped += 1;
            continue;
        }

        let total_books = book_rows.len() as i32;
        let volumes_done = (*progress_volumes).min(total_books);

        for (idx, book_row) in book_rows.iter().enumerate() {
            let book_id: Uuid = book_row.get("id");
            let book_status = if local_status == "read" || (idx as i32) < volumes_done {
                "read"
            } else if local_status == "reading" && idx as i32 == volumes_done {
                "reading"
            } else {
                "unread"
            };

            let _ = sqlx::query(
                r#"
                INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
                VALUES ($1, $3, $2, NULL, NOW(), NOW())
                ON CONFLICT (book_id, user_id) DO UPDATE
                  SET status = EXCLUDED.status, updated_at = NOW()
                  WHERE book_reading_progress.status != EXCLUDED.status
                "#,
            )
            .bind(book_id)
            .bind(book_status)
            .bind(local_user_id)
            .execute(&state.pool)
            .await;
        }

        items.push(AnilistPullItem {
            series_name: series_name.clone(),
            anilist_title: anilist_title.clone(),
            anilist_url: anilist_url.clone(),
            anilist_status: anilist_status.clone(),
            books_updated: total_books,
        });
        updated += 1;
    }

    Ok(Json(AnilistPullReport { updated, skipped, errors, items }))
}

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
        "SELECT library_id, series_name, anilist_id, anilist_title, anilist_url, status, linked_at, synced_at
         FROM anilist_series_links
         ORDER BY linked_at DESC",
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
