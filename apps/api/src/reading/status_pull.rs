use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, integrations::anilist, state::AppState};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct ReadingStatusPullReportDto {
    #[schema(value_type = String)]
    pub job_id: Uuid,
    pub status: String,
    pub total_linked: i64,
    pub updated: i64,
    pub unrated: i64,
    pub not_found: i64,
}

// ---------------------------------------------------------------------------
// POST /ratings/pull — Trigger an AniList score pull job
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/ratings/pull",
    tag = "reading_status",
    responses(
        (status = 200, description = "Job created"),
        (status = 400, description = "AniList not configured"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_pull(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (_, _, local_user_id) = anilist::load_anilist_settings(&state.pool).await?;
    if local_user_id.is_none() {
        return Err(ApiError::bad_request(
            "AniList local_user_id not configured — required for reading status pull",
        ));
    }

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE type = 'rating_pull' AND status IN ('pending', 'running') LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some(existing_id) = existing {
        return Ok(Json(serde_json::json!({
            "id": existing_id.to_string(),
            "status": "already_running",
        })));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, type, status, started_at) VALUES ($1, 'rating_pull', 'running', NOW())",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Err(e) = process_rating_pull(&pool, job_id).await {
            warn!("[RATING_PULL] job {job_id} failed: {e}");
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .bind(e.to_string())
            .execute(&pool)
            .await;
        }
    });

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "running",
    })))
}

// ---------------------------------------------------------------------------
// GET /ratings/pull/:id/report
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/ratings/pull/{id}/report",
    tag = "reading_status",
    params(("id" = String, Path, description = "Job UUID")),
    responses(
        (status = 200, body = ReadingStatusPullReportDto),
        (status = 404, description = "Job not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_pull_report(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<Uuid>,
) -> Result<Json<ReadingStatusPullReportDto>, ApiError> {
    let row = sqlx::query(
        "SELECT status, stats_json FROM index_jobs WHERE id = $1 AND type = 'rating_pull'",
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("job not found"))?;

    let status: String = row.get("status");
    let stats: Option<serde_json::Value> = row.get("stats_json");

    let (total_linked, updated, unrated, not_found) = stats
        .as_ref()
        .map(|s| {
            (
                s["total_linked"].as_i64().unwrap_or(0),
                s["updated"].as_i64().unwrap_or(0),
                s["unrated"].as_i64().unwrap_or(0),
                s["not_found"].as_i64().unwrap_or(0),
            )
        })
        .unwrap_or((0, 0, 0, 0));

    Ok(Json(ReadingStatusPullReportDto {
        job_id,
        status,
        total_linked,
        updated,
        unrated,
        not_found,
    }))
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

pub async fn process_rating_pull(pool: &PgPool, job_id: Uuid) -> Result<(), String> {
    let (token, user_id, local_user_id_opt) = anilist::load_anilist_settings(pool)
        .await
        .map_err(|e| e.message)?;

    let user_id = user_id.ok_or_else(|| "AniList user_id not configured".to_string())?;
    let local_user_id =
        local_user_id_opt.ok_or_else(|| "AniList local_user_id not configured".to_string())?;

    // Fetch full manga list from AniList → build anilist_id → score map
    let gql = r#"
        query GetUserMangaList($userId: Int) {
            MediaListCollection(userId: $userId, type: MANGA) {
                lists {
                    entries {
                        media { id }
                        score(format: POINT_100)
                    }
                }
            }
        }
    "#;

    let data = anilist::anilist_graphql(&token, gql, serde_json::json!({ "userId": user_id }))
        .await
        .map_err(|e| e.message)?;

    let mut score_map: std::collections::HashMap<i32, Option<f64>> =
        std::collections::HashMap::new();
    if let Some(lists) = data["MediaListCollection"]["lists"].as_array() {
        for list in lists {
            if let Some(entries) = list["entries"].as_array() {
                for entry in entries {
                    let media_id = entry["media"]["id"].as_i64().unwrap_or(0) as i32;
                    // score = 0 means not rated on AniList
                    let score = entry["score"].as_f64().filter(|&s| s > 0.0);
                    score_map.insert(media_id, score);
                }
            }
        }
    }

    // Query all locally linked series (all libraries with AniList provider)
    let link_rows = sqlx::query(
        r#"
        SELECT asl.series_id, s.name AS series_name, asl.anilist_id
        FROM anilist_series_links asl
        JOIN series s ON s.id = asl.series_id
        JOIN libraries l ON l.id = asl.library_id
        WHERE l.reading_status_provider = 'anilist'
          AND asl.anilist_id IS NOT NULL
        ORDER BY s.name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let total = link_rows.len() as i32;
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut count_updated = 0i64;
    let mut count_unrated = 0i64;
    let mut count_not_found = 0i64;

    for (processed, row) in link_rows.iter().enumerate() {
        let series_id: Uuid = row.get("series_id");
        let series_name: String = row.get("series_name");
        let anilist_id: i32 = row.get("anilist_id");

        match score_map.get(&anilist_id) {
            None => {
                // Series linked locally but not in user's AniList list
                count_not_found += 1;
                insert_event(pool, job_id, "not_found", "info", Some(&series_name), None, None).await;
            }
            Some(score_100_opt) => {
                let user_score = score_100_opt.map(|s| s / 10.0);

                // Update user_score on anilist_series_links
                let _ = sqlx::query(
                    "UPDATE anilist_series_links SET user_score = $1 WHERE series_id = $2",
                )
                .bind(user_score)
                .bind(series_id)
                .execute(pool)
                .await;

                if let Some(score) = user_score {
                    let rounded = (score.round() as i16).clamp(1, 10);
                    // Backfill local rating — DO NOTHING if already set manually
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
                    .execute(pool)
                    .await;

                    count_updated += 1;
                    insert_event(
                        pool,
                        job_id,
                        "score_updated",
                        "info",
                        Some(&series_name),
                        None,
                        Some(serde_json::json!({ "score_100": score_100_opt, "rounded": rounded })),
                    )
                    .await;
                } else {
                    count_unrated += 1;
                    insert_event(pool, job_id, "unrated", "info", Some(&series_name), None, None).await;
                }
            }
        }

        let pct = if total > 0 {
            ((processed + 1) as f32 / total as f32 * 100.0) as i32
        } else {
            100
        };
        let _ = sqlx::query(
            "UPDATE index_jobs SET processed_files = $2, progress_percent = $3 WHERE id = $1",
        )
        .bind(job_id)
        .bind((processed + 1) as i32)
        .bind(pct)
        .execute(pool)
        .await;
    }

    let stats = serde_json::json!({
        "total_linked": total as i64,
        "updated": count_updated,
        "unrated": count_unrated,
        "not_found": count_not_found,
    });

    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), stats_json = $2, progress_percent = 100 WHERE id = $1",
    )
    .bind(job_id)
    .bind(&stats)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    info!(
        "[RATING_PULL] job={job_id} done: {total} linked, updated={count_updated}, unrated={count_unrated}, not_found={count_not_found}"
    );

    Ok(())
}

async fn insert_event(
    pool: &PgPool,
    job_id: Uuid,
    event_type: &str,
    level: &str,
    entity_name: Option<&str>,
    message: Option<&str>,
    detail: Option<serde_json::Value>,
) {
    let _ = sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name, message, detail) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(job_id)
    .bind(event_type)
    .bind(level)
    .bind(entity_name)
    .bind(message)
    .bind(detail)
    .execute(pool)
    .await;
}
