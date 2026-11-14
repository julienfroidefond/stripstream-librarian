use axum::extract::State;
use axum::Json;
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use super::anilist::{anilist_graphql, load_anilist_settings};
use super::anilist_rating_push::anilist_score_to_local;
use crate::{error::ApiError, state::AppState};

// ─── Types ────────────────────────────────────────────────────────────────────

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

// ─── Handlers ─────────────────────────────────────────────────────────────────

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
    let local_user_id = local_user_id.ok_or_else(|| {
        ApiError::bad_request(
            "AniList local user not configured — please select a user in settings",
        )
    })?;

    let links = sqlx::query(
        r#"
        SELECT asl.library_id, asl.series_id, s.name AS series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url
        FROM anilist_series_links asl
        JOIN series s ON s.id = asl.series_id
        JOIN libraries l ON l.id = asl.library_id
        WHERE l.reading_status_provider = 'anilist'
        ORDER BY l.name, s.name
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut items: Vec<AnilistSyncPreviewItem> = Vec::new();

    for link in &links {
        let series_id: Uuid = link.get("series_id");
        let series_name: String = link.get("series_name");
        let anilist_id: i32 = link.get("anilist_id");
        let anilist_title: Option<String> = link.get("anilist_title");
        let anilist_url: Option<String> = link.get("anilist_url");

        let Ok(stats) = series_reading_stats(&state.pool, series_id, local_user_id).await else {
            continue;
        };
        if stats.book_count == 0 {
            continue;
        }
        let (status, progress_volumes) = stats.anilist_progress();

        items.push(AnilistSyncPreviewItem {
            series_name,
            anilist_id,
            anilist_title,
            anilist_url,
            status: status.to_string(),
            progress_volumes,
            books_read: stats.books_read,
            book_count: stats.book_count,
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
    let local_user_id = local_user_id.ok_or_else(|| {
        ApiError::bad_request(
            "AniList local user not configured — please select a user in settings",
        )
    })?;

    let links = sqlx::query(
        r#"
        SELECT asl.library_id, asl.series_id, s.name AS series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url
        FROM anilist_series_links asl
        JOIN series s ON s.id = asl.series_id
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
        let series_id: Uuid = link.get("series_id");
        let series_name: String = link.get("series_name");
        let anilist_id: i32 = link.get("anilist_id");
        let anilist_title: Option<String> = link.get("anilist_title");
        let anilist_url: Option<String> = link.get("anilist_url");

        let stats = match series_reading_stats(&state.pool, series_id, local_user_id).await {
            Ok(stats) => stats,
            Err(e) => {
                errors.push(format!("{series_name}: DB error: {e}"));
                continue;
            }
        };
        if stats.book_count == 0 {
            skipped += 1;
            continue;
        }
        let (status, progress_volumes) = stats.anilist_progress();

        let vars = serde_json::json!({
            "mediaId": anilist_id,
            "status": status,
            "progressVolumes": progress_volumes,
        });

        match anilist_graphql(&token, gql_update, vars).await {
            Ok(_) => {
                let _ = sqlx::query(
                    "UPDATE anilist_series_links SET status = 'synced', synced_at = NOW() WHERE library_id = $1 AND series_id = $2",
                )
                .bind(link.get::<Uuid, _>("library_id"))
                .bind(series_id)
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
                    "UPDATE anilist_series_links SET status = 'error' WHERE library_id = $1 AND series_id = $2",
                )
                .bind(link.get::<Uuid, _>("library_id"))
                .bind(series_id)
                .execute(&state.pool)
                .await;
                errors.push(format!("{series_name}: {}", e.message));
            }
        }
    }

    Ok(Json(AnilistSyncReport {
        synced,
        skipped,
        errors,
        items,
    }))
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
    let user_id = user_id.ok_or_else(|| {
        ApiError::bad_request(
            "AniList user_id not configured — please test the connection in settings",
        )
    })?;
    let local_user_id = local_user_id.ok_or_else(|| {
        ApiError::bad_request(
            "AniList local user not configured — please select a user in settings",
        )
    })?;

    let gql = r#"
        query GetUserMangaList($userId: Int) {
            MediaListCollection(userId: $userId, type: MANGA) {
                lists {
                    entries {
                        media { id siteUrl }
                        status
                        progressVolumes
                        score(format: POINT_100)
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

    // (anilist_media_id, status, progress_volumes, score_0_100)
    let mut entries: Vec<(i32, String, i32, Option<f64>)> = Vec::new();
    for list in &lists {
        if let Some(list_entries) = list["entries"].as_array() {
            for entry in list_entries {
                let media_id = entry["media"]["id"].as_i64().unwrap_or(0) as i32;
                let status = entry["status"].as_str().unwrap_or("").to_string();
                let progress = entry["progressVolumes"].as_i64().unwrap_or(0) as i32;
                // score is 0 when not rated on AniList — treat 0 as None
                let score = entry["score"].as_f64().filter(|&s| s > 0.0);
                entries.push((media_id, status, progress, score));
            }
        }
    }

    let link_rows = sqlx::query(
        r#"
        SELECT asl.library_id, asl.series_id, s.name AS series_name, asl.anilist_id, asl.anilist_title, asl.anilist_url
        FROM anilist_series_links asl
        JOIN series s ON s.id = asl.series_id
        JOIN libraries l ON l.id = asl.library_id
        WHERE l.reading_status_provider = 'anilist'
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut link_map: std::collections::HashMap<
        i32,
        (Uuid, String, Option<String>, Option<String>),
    > = std::collections::HashMap::new();
    for row in &link_rows {
        let aid: i32 = row.get("anilist_id");
        let sid: Uuid = row.get("series_id");
        let name: String = row.get("series_name");
        let title: Option<String> = row.get("anilist_title");
        let url: Option<String> = row.get("anilist_url");
        link_map.insert(aid, (sid, name, title, url));
    }

    let mut updated = 0i32;
    let mut skipped = 0i32;
    let mut errors: Vec<String> = Vec::new();
    let mut items: Vec<AnilistPullItem> = Vec::new();

    for (anilist_id, anilist_status, progress_volumes, user_score_100) in &entries {
        let Some((series_id, series_name, anilist_title, anilist_url)) = link_map.get(anilist_id)
        else {
            skipped += 1;
            continue;
        };

        // Persist user score (normalised 0-10) on anilist_series_links
        let user_score_normalised = user_score_100.map(|s| s / 10.0);
        let _ = sqlx::query("UPDATE anilist_series_links SET user_score = $1 WHERE series_id = $2")
            .bind(user_score_normalised)
            .bind(series_id)
            .execute(&state.pool)
            .await;

        // Backfill series_user_ratings if no local rating exists yet
        if let Some(score_100) = user_score_100 {
            let rounded = anilist_score_to_local(*score_100);
            let _ = sqlx::query(
                r#"
                INSERT INTO series_user_ratings (user_id, series_id, rating)
                VALUES ($1, $2, $3)
                ON CONFLICT (user_id, series_id) DO NOTHING
                "#,
            )
            .bind(local_user_id)
            .bind(series_id)
            .bind(rounded)
            .execute(&state.pool)
            .await;
        }

        let local_status = match anilist_status.as_str() {
            "COMPLETED" => "read",
            "CURRENT" | "REPEATING" => "reading",
            "PLANNING" | "PAUSED" | "DROPPED" => "unread",
            _ => {
                skipped += 1;
                continue;
            }
        };

        let books_updated = match apply_pulled_progress(
            &state.pool,
            *series_id,
            local_user_id,
            local_status,
            *progress_volumes,
        )
        .await
        {
            Ok(Some(count)) => count,
            Ok(None) => {
                skipped += 1;
                continue;
            }
            Err(e) => {
                errors.push(format!("{series_name}: {e}"));
                continue;
            }
        };

        items.push(AnilistPullItem {
            series_name: series_name.clone(),
            anilist_title: anilist_title.clone(),
            anilist_url: anilist_url.clone(),
            anilist_status: anilist_status.clone(),
            books_updated,
        });
        updated += 1;
    }

    state.stats_cache.invalidate();

    Ok(Json(AnilistPullReport {
        updated,
        skipped,
        errors,
        items,
    }))
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Local reading stats of a series for the AniList user, on `regular` volumes
/// only (HS / oneshot / integral do not take part in tome numbering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SeriesReadingStats {
    pub book_count: i64,
    pub books_read: i64,
    pub total_volumes: Option<i32>,
}

impl SeriesReadingStats {
    /// AniList status and `progressVolumes` derived from the local stats.
    pub(crate) fn anilist_progress(&self) -> (&'static str, i32) {
        if self.books_read > 0
            && self
                .total_volumes
                .is_some_and(|tv| self.books_read >= tv as i64)
        {
            ("COMPLETED", self.books_read as i32)
        } else if self.books_read > 0 {
            ("CURRENT", self.books_read as i32)
        } else {
            ("PLANNING", 0)
        }
    }
}

pub(crate) async fn series_reading_stats(
    pool: &sqlx::PgPool,
    series_id: Uuid,
    local_user_id: Uuid,
) -> Result<SeriesReadingStats, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) as book_count,
            COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read,
            (SELECT sm.total_volumes FROM series sm WHERE sm.id = $1 LIMIT 1) as total_volumes
        FROM books b
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = $2
        -- Same scope as the status push (reading/status_push.rs): integrals
        -- count towards completion, specials and one-shots never do.
        WHERE b.series_id = $1 AND b.volume_type IN ('regular', 'integral')
        "#,
    )
    .bind(series_id)
    .bind(local_user_id)
    .fetch_one(pool)
    .await?;
    Ok(SeriesReadingStats {
        book_count: row.get("book_count"),
        books_read: row.get("books_read"),
        total_volumes: row.get("total_volumes"),
    })
}

/// Rank of a local reading status: a pull only ever moves a book forward.
fn status_rank(status: &str) -> u8 {
    match status {
        "read" => 2,
        "reading" => 1,
        _ => 0,
    }
}

/// Target status of each book (in volume order) for an AniList entry mapped to
/// `local_status` with `progress_volumes` volumes read.
pub(crate) fn pulled_book_statuses(
    local_status: &str,
    progress_volumes: i32,
    book_count: usize,
) -> Vec<&'static str> {
    let volumes_done = progress_volumes.clamp(0, book_count as i32) as usize;
    (0..book_count)
        .map(|idx| {
            if local_status == "read" || idx < volumes_done {
                "read"
            } else if local_status == "reading" && idx == volumes_done {
                "reading"
            } else {
                "unread"
            }
        })
        .collect()
}

/// Apply a pulled AniList entry to the local progress of the `regular` books of
/// a series. Only advances are written (unread → reading → read): a book read
/// locally is never moved back, whatever AniList reports. Returns the number of
/// books updated, or `None` when the series has no regular book.
pub(crate) async fn apply_pulled_progress(
    pool: &sqlx::PgPool,
    series_id: Uuid,
    local_user_id: Uuid,
    local_status: &str,
    progress_volumes: i32,
) -> Result<Option<i32>, sqlx::Error> {
    let book_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT b.id FROM books b \
         WHERE b.series_id = $1 AND b.volume_type = 'regular' \
         ORDER BY b.volume NULLS LAST, b.title, b.id",
    )
    .bind(series_id)
    .fetch_all(pool)
    .await?;
    if book_ids.is_empty() {
        return Ok(None);
    }

    let statuses = pulled_book_statuses(local_status, progress_volumes, book_ids.len());
    let (ids, targets): (Vec<Uuid>, Vec<String>) = book_ids
        .into_iter()
        .zip(statuses)
        .filter(|(_, status)| status_rank(status) > 0)
        .map(|(id, status)| (id, status.to_string()))
        .unzip();
    if ids.is_empty() {
        return Ok(Some(0));
    }

    let result = sqlx::query(
        r#"
        INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
        SELECT t.book_id, $3, t.status, NULL, NOW(), NOW()
        FROM UNNEST($1::uuid[], $2::text[]) AS t(book_id, status)
        ON CONFLICT (book_id, user_id) DO UPDATE
          SET status = EXCLUDED.status, updated_at = NOW()
          WHERE (CASE book_reading_progress.status WHEN 'read' THEN 2 WHEN 'reading' THEN 1 ELSE 0 END)
              < (CASE EXCLUDED.status WHEN 'read' THEN 2 WHEN 'reading' THEN 1 ELSE 0 END)
        "#,
    )
    .bind(&ids)
    .bind(&targets)
    .bind(local_user_id)
    .execute(pool)
    .await?;
    Ok(Some(result.rows_affected() as i32))
}

#[cfg(test)]
#[path = "tests/anilist_sync.rs"]
mod tests;
