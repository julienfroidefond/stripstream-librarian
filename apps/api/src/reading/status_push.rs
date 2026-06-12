use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, integrations::anilist, state::AppState};

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
    #[schema(value_type = Option<String>)]
    pub series_id: Option<Uuid>,
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
            "SELECT id FROM libraries WHERE reading_status_provider = 'anilist' ORDER BY name",
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
            if existing.is_some() {
                continue;
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
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await?;

    let mut pushed = 0i64;
    let mut skipped = 0i64;
    let mut no_books = 0i64;
    let mut errors = 0i64;

    for r in &counts {
        let event_type: String = r.get("event_type");
        let cnt: i64 = r.get("cnt");
        match event_type.as_str() {
            "status_pushed" => pushed = cnt,
            "status_skipped" => skipped = cnt,
            "status_no_books" => no_books = cnt,
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
    let job_library_id: Option<Uuid> =
        sqlx::query_scalar("SELECT library_id FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();

    // Map frontend status values to event_type values
    let event_type_filter = query.status.as_deref().map(|s| match s {
        "pushed" => "status_pushed",
        "skipped" => "status_skipped",
        "no_books" => "status_no_books",
        "error" => "error",
        other => other,
    });

    let rows = if let Some(et_filter) = event_type_filter {
        sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $3 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1 AND e.event_type = $2
             ORDER BY e.entity_name",
        )
        .bind(job_id)
        .bind(et_filter)
        .bind(job_library_id)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(job_library_id)
        .fetch_all(&state.pool)
        .await?
    };

    let results = rows
        .iter()
        .map(|row| {
            let event_type: String = row.get("event_type");
            let detail: Option<serde_json::Value> = row.get("detail");
            let status = match event_type.as_str() {
                "status_pushed" => "pushed",
                "status_skipped" => "skipped",
                "status_no_books" => "no_books",
                "error" => "error",
                other => other,
            };
            ReadingStatusPushResultDto {
                id: row.get("id"),
                series_id: row.get("series_id"),
                series_name: row
                    .get::<Option<String>, _>("entity_name")
                    .unwrap_or_default(),
                status: status.to_string(),
                anilist_id: detail
                    .as_ref()
                    .and_then(|d| d["anilist_id"].as_i64())
                    .map(|v| v as i32),
                anilist_title: detail
                    .as_ref()
                    .and_then(|d| d["anilist_title"].as_str().map(String::from)),
                anilist_url: detail
                    .as_ref()
                    .and_then(|d| d["anilist_url"].as_str().map(String::from)),
                anilist_status: detail
                    .as_ref()
                    .and_then(|d| d["anilist_status"].as_str().map(String::from)),
                progress_volumes: detail
                    .as_ref()
                    .and_then(|d| d["progress"].as_i64())
                    .map(|v| v as i32),
                error_message: row.get("message"),
            }
        })
        .collect();

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

struct SeriesInfo {
    series_id: Uuid,
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

    let local_user_id =
        local_user_id_opt.ok_or_else(|| "AniList local_user_id not configured".to_string())?;

    // Find all linked series that need a push (differential)
    let series_to_push: Vec<SeriesInfo> = sqlx::query(
        r#"
        SELECT
            asl.series_id,
            s.name AS series_name,
            asl.anilist_id,
            asl.anilist_title,
            asl.anilist_url
        FROM anilist_series_links asl
        JOIN series s ON s.id = asl.series_id
        WHERE asl.library_id = $1
          AND asl.anilist_id IS NOT NULL
          AND (
            asl.synced_at IS NULL
            OR EXISTS (
                SELECT 1
                FROM book_reading_progress brp
                JOIN books b2 ON b2.id = brp.book_id
                WHERE b2.series_id = asl.series_id
                  AND brp.user_id = $2
                  AND brp.updated_at > asl.synced_at
            )
            OR EXISTS (
                SELECT 1
                FROM books b2
                WHERE b2.series_id = asl.series_id
                  AND b2.created_at > asl.synced_at
            )
          )
        ORDER BY s.name
        "#,
    )
    .bind(library_id)
    .bind(local_user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(|row| SeriesInfo {
        series_id: row.get("series_id"),
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
            LEFT JOIN book_reading_progress brp
                ON brp.book_id = b.id AND brp.user_id = $2
            WHERE b.series_id = $1
              AND b.volume_type IN ('regular', 'integral')
            "#,
        )
        .bind(series.series_id)
        .bind(local_user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

        let total_books: i64 = stats_row.get("total_books");
        let books_read: i64 = stats_row.get("books_read");

        if total_books == 0 {
            insert_event(pool, job_id, "status_no_books", "info", Some(&series.series_name), None, Some(serde_json::json!({"anilist_id": series.anilist_id, "anilist_title": series.anilist_title, "anilist_url": series.anilist_url}))).await;
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

        match push_to_anilist(&token, series.anilist_id, anilist_status, progress_volumes).await {
            Ok(()) => {
                // Update synced_at
                let _ = sqlx::query(
                    "UPDATE anilist_series_links SET synced_at = NOW() WHERE series_id = $1",
                )
                .bind(series.series_id)
                .execute(pool)
                .await;

                insert_event(pool, job_id, "status_pushed", "info", Some(&series.series_name), None, Some(serde_json::json!({"anilist_id": series.anilist_id, "anilist_title": series.anilist_title, "anilist_url": series.anilist_url, "anilist_status": anilist_status, "progress": progress_volumes}))).await;
            }
            Err(e) if e.contains("429") || e.contains("Too Many Requests") => {
                warn!(
                    "[READING_STATUS_PUSH] rate limit hit for '{}', waiting 10s before retry",
                    series.series_name
                );
                tokio::time::sleep(Duration::from_secs(10)).await;
                match push_to_anilist(&token, series.anilist_id, anilist_status, progress_volumes)
                    .await
                {
                    Ok(()) => {
                        let _ = sqlx::query(
                            "UPDATE anilist_series_links SET synced_at = NOW() WHERE series_id = $1",
                        )
                        .bind(series.series_id)
                        .execute(pool)
                        .await;

                        insert_event(pool, job_id, "status_pushed", "info", Some(&series.series_name), None, Some(serde_json::json!({"anilist_id": series.anilist_id, "anilist_title": series.anilist_title, "anilist_url": series.anilist_url, "anilist_status": anilist_status, "progress": progress_volumes}))).await;
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
                insert_event(pool, job_id, "error", "error", Some(&series.series_name), Some(&e), Some(serde_json::json!({"anilist_id": series.anilist_id, "anilist_title": series.anilist_title, "anilist_url": series.anilist_url}))).await;
            }
        }

        // Respect AniList rate limit (~90 req/min)
        tokio::time::sleep(Duration::from_millis(700)).await;
    }

    // Build final stats from events
    let counts = sqlx::query(
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
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
        let s: String = row.get("event_type");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "status_pushed" => count_pushed = c,
            "status_skipped" => count_skipped = c,
            "status_no_books" => count_no_books = c,
            "error" => count_errors = c,
            _ => {}
        }
    }

    let pushed_series_names: Vec<String> = sqlx::query_scalar(
        "SELECT entity_name FROM index_job_events \
         WHERE job_id = $1 AND event_type = 'status_pushed' AND entity_name IS NOT NULL \
         ORDER BY entity_name \
         LIMIT 10",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

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

    let library_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
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
            skipped: count_skipped,
            no_books: count_no_books,
            errors: count_errors,
            pushed_series_names,
        },
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

async fn build_push_stats(pool: &PgPool, job_id: Uuid) -> serde_json::Value {
    let total: Option<i32> = sqlx::query_scalar("SELECT total_files FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    let counts = sqlx::query(
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
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
        let s: String = row.get("event_type");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "status_pushed" => pushed = c,
            "status_skipped" => skipped = c,
            "status_no_books" => no_books = c,
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

#[cfg(test)]
#[path = "tests/status_push.rs"]
mod tests;
