use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{anilist, error::ApiError, state::AppState};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct ReadingStatusPushRequest {
    pub library_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingStatusPushReportDto {
    #[schema(value_type = String)]
    pub job_id: Uuid,
    pub status: String,
    pub total_series: i64,
    pub pushed: i64,
    pub skipped: i64,
    pub no_books: i64,
    pub errors: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingStatusPushResultDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub series_name: String,
    /// 'pushed' | 'skipped' | 'no_books' | 'error'
    pub status: String,
    pub anilist_id: Option<i32>,
    pub anilist_title: Option<String>,
    pub anilist_url: Option<String>,
    /// PLANNING | CURRENT | COMPLETED
    pub anilist_status: Option<String>,
    pub progress_volumes: Option<i32>,
    pub error_message: Option<String>,
}

// ---------------------------------------------------------------------------
// POST /reading-status/push — Trigger a reading status push job
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/reading-status/push",
    tag = "reading_status",
    request_body = ReadingStatusPushRequest,
    responses(
        (status = 200, description = "Job created"),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_push(
    State(state): State<AppState>,
    Json(body): Json<ReadingStatusPushRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // All libraries case
    if body.library_id.is_none() {
        let (_, _, local_user_id) = anilist::load_anilist_settings(&state.pool).await?;
        if local_user_id.is_none() {
            return Err(ApiError::bad_request(
                "AniList local_user_id not configured — required for reading status push",
            ));
        }
        let library_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM libraries WHERE reading_status_provider = 'anilist' ORDER BY name"
        )
        .fetch_all(&state.pool)
        .await?;
        let mut last_job_id: Option<Uuid> = None;
        for library_id in library_ids {
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'reading_status_push' AND status IN ('pending', 'running') LIMIT 1",
            )
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?;
            if existing.is_some() { continue; }
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_push', 'running', NOW())",
            )
            .bind(job_id)
            .bind(library_id)
            .execute(&state.pool)
            .await?;
            let pool = state.pool.clone();
            let library_name: Option<String> = sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
                .bind(library_id)
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten();
            tokio::spawn(async move {
                if let Err(e) = process_reading_status_push(&pool, job_id, library_id).await {
                    warn!("[READING_STATUS_PUSH] job {job_id} failed: {e}");
                    let partial_stats = build_push_stats(&pool, job_id).await;
                    let _ = sqlx::query(
                        "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW(), stats_json = $3 WHERE id = $1",
                    )
                    .bind(job_id)
                    .bind(e.to_string())
                    .bind(&partial_stats)
                    .execute(&pool)
                    .await;
                    notifications::notify(
                        pool.clone(),
                        notifications::NotificationEvent::ReadingStatusPushFailed {
                            library_name,
                            error: e.to_string(),
                        },
                    );
                }
            });
            last_job_id = Some(job_id);
        }
        return Ok(Json(serde_json::json!({
            "id": last_job_id.map(|id| id.to_string()),
            "status": "started",
        })));
    }

    let library_id: Uuid = body
        .library_id
        .unwrap()
        .parse()
        .map_err(|_| ApiError::bad_request("invalid library_id"))?;

    // Verify library exists and has AniList configured
    let lib_row = sqlx::query("SELECT reading_status_provider FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("library not found"))?;

    let provider: Option<String> = lib_row.get("reading_status_provider");
    if provider.as_deref() != Some("anilist") {
        return Err(ApiError::bad_request(
            "This library has no AniList reading status provider configured",
        ));
    }

    // Check AniList is configured globally with a local_user_id
    let (_, _, local_user_id) = anilist::load_anilist_settings(&state.pool).await?;
    if local_user_id.is_none() {
        return Err(ApiError::bad_request(
            "AniList local_user_id not configured — required for reading status push",
        ));
    }

    // Check no existing running job for this library
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'reading_status_push' AND status IN ('pending', 'running') LIMIT 1",
    )
    .bind(library_id)
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
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_push', 'running', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    let pool = state.pool.clone();
    let library_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();

    tokio::spawn(async move {
        if let Err(e) = process_reading_status_push(&pool, job_id, library_id).await {
            warn!("[READING_STATUS_PUSH] job {job_id} failed: {e}");
            let partial_stats = build_push_stats(&pool, job_id).await;
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW(), stats_json = $3 WHERE id = $1",
            )
            .bind(job_id)
            .bind(e.to_string())
            .bind(&partial_stats)
            .execute(&pool)
            .await;
            notifications::notify(
                pool.clone(),
                notifications::NotificationEvent::ReadingStatusPushFailed {
                    library_name,
                    error: e.to_string(),
                },
            );
        }
    });

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "running",
    })))
}

// ---------------------------------------------------------------------------
// GET /reading-status/push/:id/report
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/reading-status/push/{id}/report",
    tag = "reading_status",
    params(("id" = String, Path, description = "Job UUID")),
    responses(
        (status = 200, body = ReadingStatusPushReportDto),
        (status = 404, description = "Job not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_push_report(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<Uuid>,
) -> Result<Json<ReadingStatusPushReportDto>, ApiError> {
    let row = sqlx::query(
        "SELECT status, total_files FROM index_jobs WHERE id = $1 AND type = 'reading_status_push'",
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("job not found"))?;

    let job_status: String = row.get("status");
    let total_files: Option<i32> = row.get("total_files");

    let counts = sqlx::query(
        "SELECT status, COUNT(*) as cnt FROM reading_status_push_results WHERE job_id = $1 GROUP BY status",
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await?;

    let mut pushed = 0i64;
    let mut skipped = 0i64;
    let mut no_books = 0i64;
    let mut errors = 0i64;

    for r in &counts {
        let status: String = r.get("status");
        let cnt: i64 = r.get("cnt");
        match status.as_str() {
            "pushed" => pushed = cnt,
            "skipped" => skipped = cnt,
            "no_books" => no_books = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    Ok(Json(ReadingStatusPushReportDto {
        job_id,
        status: job_status,
        total_series: total_files.unwrap_or(0) as i64,
        pushed,
        skipped,
        no_books,
        errors,
    }))
}

// ---------------------------------------------------------------------------
// GET /reading-status/push/:id/results
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PushResultsQuery {
    pub status: Option<String>,
}

#[utoipa::path(
    get,
    path = "/reading-status/push/{id}/results",
    tag = "reading_status",
    params(
        ("id" = String, Path, description = "Job UUID"),
        ("status" = Option<String>, Query, description = "Filter by status"),
    ),
    responses(
        (status = 200, body = Vec<ReadingStatusPushResultDto>),
    ),
    security(("Bearer" = []))
)]
pub async fn get_push_results(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<PushResultsQuery>,
) -> Result<Json<Vec<ReadingStatusPushResultDto>>, ApiError> {
    let rows = if let Some(status_filter) = &query.status {
        sqlx::query(
            "SELECT id, series_name, status, anilist_id, anilist_title, anilist_url, anilist_status, progress_volumes, error_message
             FROM reading_status_push_results
             WHERE job_id = $1 AND status = $2
             ORDER BY series_name",
        )
        .bind(job_id)
        .bind(status_filter)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, series_name, status, anilist_id, anilist_title, anilist_url, anilist_status, progress_volumes, error_message
             FROM reading_status_push_results
             WHERE job_id = $1
             ORDER BY status, series_name",
        )
        .bind(job_id)
        .fetch_all(&state.pool)
        .await?
    };

    let results = rows
        .iter()
        .map(|row| ReadingStatusPushResultDto {
            id: row.get("id"),
            series_name: row.get("series_name"),
            status: row.get("status"),
            anilist_id: row.get("anilist_id"),
            anilist_title: row.get("anilist_title"),
            anilist_url: row.get("anilist_url"),
            anilist_status: row.get("anilist_status"),
            progress_volumes: row.get("progress_volumes"),
            error_message: row.get("error_message"),
        })
        .collect();

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

struct SeriesInfo {
    series_name: String,
    anilist_id: i32,
    anilist_title: Option<String>,
    anilist_url: Option<String>,
}

pub async fn process_reading_status_push(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(), String> {
    let (token, _, local_user_id_opt) = anilist::load_anilist_settings(pool)
        .await
        .map_err(|e| e.message)?;

    let local_user_id = local_user_id_opt
        .ok_or_else(|| "AniList local_user_id not configured".to_string())?;

    // Find all linked series that need a push (differential)
    let series_to_push: Vec<SeriesInfo> = sqlx::query(
        r#"
        SELECT
            asl.series_name,
            asl.anilist_id,
            asl.anilist_title,
            asl.anilist_url
        FROM anilist_series_links asl
        WHERE asl.library_id = $1
          AND asl.anilist_id IS NOT NULL
          AND (
            asl.synced_at IS NULL
            OR EXISTS (
                SELECT 1
                FROM book_reading_progress brp
                JOIN books b2 ON b2.id = brp.book_id
                LEFT JOIN series s2 ON s2.id = b2.series_id
                WHERE b2.library_id = asl.library_id
                  AND COALESCE(s2.name, 'unclassified') = asl.series_name
                  AND brp.user_id = $2
                  AND brp.updated_at > asl.synced_at
            )
            OR EXISTS (
                SELECT 1
                FROM books b2
                LEFT JOIN series s2 ON s2.id = b2.series_id
                WHERE b2.library_id = asl.library_id
                  AND COALESCE(s2.name, 'unclassified') = asl.series_name
                  AND b2.created_at > asl.synced_at
            )
          )
        ORDER BY asl.series_name
        "#,
    )
    .bind(library_id)
    .bind(local_user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(|row| SeriesInfo {
        series_name: row.get("series_name"),
        anilist_id: row.get("anilist_id"),
        anilist_title: row.get("anilist_title"),
        anilist_url: row.get("anilist_url"),
    })
    .collect();

    let total = series_to_push.len() as i32;
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut processed = 0i32;

    for series in &series_to_push {
        if is_job_cancelled(pool, job_id).await {
            sqlx::query(
                "UPDATE index_jobs SET status = 'cancelled', finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            return Ok(());
        }

        processed += 1;
        let progress = (processed * 100 / total.max(1)).min(100);
        sqlx::query(
            "UPDATE index_jobs SET processed_files = $2, progress_percent = $3, current_file = $4 WHERE id = $1",
        )
        .bind(job_id)
        .bind(processed)
        .bind(progress)
        .bind(&series.series_name)
        .execute(pool)
        .await
        .ok();

        // Compute reading status for this series
        let stats_row = sqlx::query(
            r#"
            SELECT
                COUNT(b.id) AS total_books,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') AS books_read
            FROM books b
            LEFT JOIN series s ON s.id = b.series_id
            LEFT JOIN book_reading_progress brp
                ON brp.book_id = b.id AND brp.user_id = $3
            WHERE b.library_id = $1
              AND COALESCE(s.name, 'unclassified') = $2
            "#,
        )
        .bind(library_id)
        .bind(&series.series_name)
        .bind(local_user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

        let total_books: i64 = stats_row.get("total_books");
        let books_read: i64 = stats_row.get("books_read");

        if total_books == 0 {
            insert_push_result(
                pool, job_id, library_id, &series.series_name, "no_books",
                Some(series.anilist_id), series.anilist_title.as_deref(), series.anilist_url.as_deref(),
                None, None, None,
            ).await;
            tokio::time::sleep(Duration::from_millis(700)).await;
            continue;
        }

        let anilist_status = if books_read == 0 {
            "PLANNING"
        } else if books_read >= total_books {
            "COMPLETED"
        } else {
            "CURRENT"
        };
        let progress_volumes = books_read as i32;

        match push_to_anilist(
            &token,
            series.anilist_id,
            anilist_status,
            progress_volumes,
        )
        .await
        {
            Ok(()) => {
                // Update synced_at
                let _ = sqlx::query(
                    "UPDATE anilist_series_links SET synced_at = NOW() WHERE library_id = $1 AND series_name = $2",
                )
                .bind(library_id)
                .bind(&series.series_name)
                .execute(pool)
                .await;

                insert_push_result(
                    pool, job_id, library_id, &series.series_name, "pushed",
                    Some(series.anilist_id), series.anilist_title.as_deref(), series.anilist_url.as_deref(),
                    Some(anilist_status), Some(progress_volumes), None,
                ).await;
            }
            Err(e) if e.contains("429") || e.contains("Too Many Requests") => {
                warn!("[READING_STATUS_PUSH] rate limit hit for '{}', waiting 10s before retry", series.series_name);
                tokio::time::sleep(Duration::from_secs(10)).await;
                match push_to_anilist(&token, series.anilist_id, anilist_status, progress_volumes).await {
                    Ok(()) => {
                        let _ = sqlx::query(
                            "UPDATE anilist_series_links SET synced_at = NOW() WHERE library_id = $1 AND series_name = $2",
                        )
                        .bind(library_id)
                        .bind(&series.series_name)
                        .execute(pool)
                        .await;

                        insert_push_result(
                            pool, job_id, library_id, &series.series_name, "pushed",
                            Some(series.anilist_id), series.anilist_title.as_deref(), series.anilist_url.as_deref(),
                            Some(anilist_status), Some(progress_volumes), None,
                        ).await;
                    }
                    Err(e2) => {
                        return Err(format!(
                            "AniList rate limit exceeded (429) — job stopped after {processed}/{total} series: {e2}"
                        ));
                    }
                }
            }
            Err(e) => {
                warn!("[READING_STATUS_PUSH] series '{}': {e}", series.series_name);
                insert_push_result(
                    pool, job_id, library_id, &series.series_name, "error",
                    Some(series.anilist_id), series.anilist_title.as_deref(), series.anilist_url.as_deref(),
                    None, None, Some(&e),
                ).await;
            }
        }

        // Respect AniList rate limit (~90 req/min)
        tokio::time::sleep(Duration::from_millis(700)).await;
    }

    // Build final stats
    let counts = sqlx::query(
        "SELECT status, COUNT(*) as cnt FROM reading_status_push_results WHERE job_id = $1 GROUP BY status",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut count_pushed = 0i64;
    let mut count_skipped = 0i64;
    let mut count_no_books = 0i64;
    let mut count_errors = 0i64;
    for row in &counts {
        let s: String = row.get("status");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "pushed" => count_pushed = c,
            "skipped" => count_skipped = c,
            "no_books" => count_no_books = c,
            "error" => count_errors = c,
            _ => {}
        }
    }

    let stats = serde_json::json!({
        "total_series": total as i64,
        "pushed": count_pushed,
        "skipped": count_skipped,
        "no_books": count_no_books,
        "errors": count_errors,
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
        "[READING_STATUS_PUSH] job={job_id} completed: {}/{} series, pushed={count_pushed}, no_books={count_no_books}, errors={count_errors}",
        processed, total
    );

    let library_name: Option<String> = sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    notifications::notify(
        pool.clone(),
        notifications::NotificationEvent::ReadingStatusPushCompleted {
            library_name,
            total_series: total,
            pushed: count_pushed as i32,
        },
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn push_to_anilist(
    token: &str,
    anilist_id: i32,
    status: &str,
    progress: i32,
) -> Result<(), String> {
    let gql = r#"
        mutation SaveMediaListEntry($mediaId: Int, $status: MediaListStatus, $progress: Int) {
            SaveMediaListEntry(mediaId: $mediaId, status: $status, progress: $progress) {
                id
                status
                progress
            }
        }
    "#;

    anilist::anilist_graphql(
        token,
        gql,
        serde_json::json!({
            "mediaId": anilist_id,
            "status": status,
            "progress": progress,
        }),
    )
    .await
    .map_err(|e| e.message)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_push_result(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    status: &str,
    anilist_id: Option<i32>,
    anilist_title: Option<&str>,
    anilist_url: Option<&str>,
    anilist_status: Option<&str>,
    progress_volumes: Option<i32>,
    error_message: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO reading_status_push_results
            (job_id, library_id, series_name, status, anilist_id, anilist_title, anilist_url, anilist_status, progress_volumes, error_message)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(job_id)
    .bind(library_id)
    .bind(series_name)
    .bind(status)
    .bind(anilist_id)
    .bind(anilist_title)
    .bind(anilist_url)
    .bind(anilist_status)
    .bind(progress_volumes)
    .bind(error_message)
    .execute(pool)
    .await;
}

async fn build_push_stats(pool: &PgPool, job_id: Uuid) -> serde_json::Value {
    let total: Option<i32> = sqlx::query_scalar("SELECT total_files FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    let counts = sqlx::query(
        "SELECT status, COUNT(*) as cnt FROM reading_status_push_results WHERE job_id = $1 GROUP BY status",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut pushed = 0i64;
    let mut skipped = 0i64;
    let mut no_books = 0i64;
    let mut errors = 0i64;
    for row in &counts {
        let s: String = row.get("status");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "pushed" => pushed = c,
            "skipped" => skipped = c,
            "no_books" => no_books = c,
            "error" => errors = c,
            _ => {}
        }
    }

    serde_json::json!({
        "total_series": total.unwrap_or(0) as i64,
        "pushed": pushed,
        "skipped": skipped,
        "no_books": no_books,
        "errors": errors,
    })
}

async fn is_job_cancelled(pool: &PgPool, job_id: Uuid) -> bool {
    sqlx::query_scalar::<_, String>("SELECT status FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .as_deref()
        == Some("cancelled")
}
