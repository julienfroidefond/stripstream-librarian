use axum::extract::State;
use axum::Json;
use serde::Serialize;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use super::anilist::{anilist_graphql, load_anilist_settings};
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

        let stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read,
                (SELECT sm.total_volumes FROM series sm WHERE sm.id = $1 LIMIT 1) as total_volumes
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = $2
            WHERE b.series_id = $1
            "#,
        )
        .bind(series_id)
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

        let (status, progress_volumes) =
            if books_read > 0 && total_volumes.is_some_and(|tv| books_read >= tv as i64) {
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

        let stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read,
                (SELECT sm.total_volumes FROM series sm WHERE sm.id = $1 LIMIT 1) as total_volumes
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = $2
            WHERE b.series_id = $1
            "#,
        )
        .bind(series_id)
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
        let _ = sqlx::query(
            "UPDATE anilist_series_links SET user_score = $1 WHERE series_id = $2",
        )
        .bind(user_score_normalised)
        .bind(series_id)
        .execute(&state.pool)
        .await;

        // Backfill series_user_ratings if no local rating exists yet
        if let Some(score) = user_score_normalised {
            let rounded = (score.round() as i16).clamp(1, 10);
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

        let book_rows = sqlx::query(
            "SELECT b.id, b.volume FROM books b WHERE b.series_id = $1 ORDER BY b.volume NULLS LAST",
        )
        .bind(series_id)
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

    Ok(Json(AnilistPullReport {
        updated,
        skipped,
        errors,
        items,
    }))
}
