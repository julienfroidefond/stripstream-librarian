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
pub struct ReadingStatusMatchRequest {
    pub library_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingStatusMatchReportDto {
    #[schema(value_type = String)]
    pub job_id: Uuid,
    pub status: String,
    pub total_series: i64,
    pub linked: i64,
    pub already_linked: i64,
    pub no_results: i64,
    pub ambiguous: i64,
    pub errors: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ReadingStatusMatchResultDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = Option<String>)]
    pub series_id: Option<Uuid>,
    pub series_name: String,
    /// 'linked' | 'already_linked' | 'no_results' | 'ambiguous' | 'error'
    pub status: String,
    pub anilist_id: Option<i32>,
    pub anilist_title: Option<String>,
    pub anilist_url: Option<String>,
    pub error_message: Option<String>,
}

// ---------------------------------------------------------------------------
// POST /reading-status/match — Trigger a reading status match job
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/reading-status/match",
    tag = "reading_status",
    request_body = ReadingStatusMatchRequest,
    responses(
        (status = 200, description = "Job created"),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_match(
    State(state): State<AppState>,
    Json(body): Json<ReadingStatusMatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // All libraries case
    if body.library_id.is_none() {
        anilist::load_anilist_settings(&state.pool).await?;
        let library_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM libraries WHERE reading_status_provider IS NOT NULL ORDER BY name"
        )
        .fetch_all(&state.pool)
        .await?;
        let mut last_job_id: Option<Uuid> = None;
        for library_id in library_ids {
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'reading_status_match' AND status IN ('pending', 'running') LIMIT 1",
            )
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?;
            if existing.is_some() { continue; }
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'running', NOW())",
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
                if let Err(e) = process_reading_status_match(&pool, job_id, library_id).await {
                    warn!("[READING_STATUS_MATCH] job {job_id} failed: {e}");
                    let partial_stats = build_match_stats(&pool, job_id).await;
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
                        notifications::NotificationEvent::ReadingStatusMatchFailed {
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

    // Verify library exists and has a reading_status_provider configured
    let lib_row = sqlx::query("SELECT reading_status_provider FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("library not found"))?;

    let provider: Option<String> = lib_row.get("reading_status_provider");
    if provider.is_none() {
        return Err(ApiError::bad_request(
            "This library has no reading status provider configured",
        ));
    }

    // Check AniList is configured globally
    anilist::load_anilist_settings(&state.pool).await?;

    // Check no existing running job for this library
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'reading_status_match' AND status IN ('pending', 'running') LIMIT 1",
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
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'running', NOW())",
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
        if let Err(e) = process_reading_status_match(&pool, job_id, library_id).await {
            warn!("[READING_STATUS_MATCH] job {job_id} failed: {e}");
            let partial_stats = build_match_stats(&pool, job_id).await;
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
                notifications::NotificationEvent::ReadingStatusMatchFailed {
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
// GET /reading-status/match/:id/report
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/reading-status/match/{id}/report",
    tag = "reading_status",
    params(("id" = String, Path, description = "Job UUID")),
    responses(
        (status = 200, body = ReadingStatusMatchReportDto),
        (status = 404, description = "Job not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_match_report(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<Uuid>,
) -> Result<Json<ReadingStatusMatchReportDto>, ApiError> {
    let row = sqlx::query(
        "SELECT status, total_files FROM index_jobs WHERE id = $1 AND type = 'reading_status_match'",
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

    let mut linked = 0i64;
    let mut already_linked = 0i64;
    let mut no_results = 0i64;
    let mut ambiguous = 0i64;
    let mut errors = 0i64;

    for r in &counts {
        let event_type: String = r.get("event_type");
        let cnt: i64 = r.get("cnt");
        match event_type.as_str() {
            "anilist_linked" => linked = cnt,
            "anilist_already_linked" => already_linked = cnt,
            "anilist_no_results" => no_results = cnt,
            "anilist_ambiguous" => ambiguous = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    Ok(Json(ReadingStatusMatchReportDto {
        job_id,
        status: job_status,
        total_series: total_files.unwrap_or(0) as i64,
        linked,
        already_linked,
        no_results,
        ambiguous,
        errors,
    }))
}

// ---------------------------------------------------------------------------
// GET /reading-status/match/:id/results
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/reading-status/match/{id}/results",
    tag = "reading_status",
    params(
        ("id" = String, Path, description = "Job UUID"),
        ("status" = Option<String>, Query, description = "Filter by status"),
    ),
    responses(
        (status = 200, body = Vec<ReadingStatusMatchResultDto>),
    ),
    security(("Bearer" = []))
)]
pub async fn get_match_results(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<ResultsQuery>,
) -> Result<Json<Vec<ReadingStatusMatchResultDto>>, ApiError> {
    let job_library_id: Option<Uuid> = sqlx::query_scalar("SELECT library_id FROM index_jobs WHERE id = $1")
        .bind(job_id).fetch_optional(&state.pool).await?.flatten();

    // Map frontend status values to event_type values
    let event_type_filter = query.status.as_deref().map(|s| match s {
        "linked" => "anilist_linked",
        "already_linked" => "anilist_already_linked",
        "no_results" => "anilist_no_results",
        "ambiguous" => "anilist_ambiguous",
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
                "anilist_linked" => "linked",
                "anilist_already_linked" => "already_linked",
                "anilist_no_results" => "no_results",
                "anilist_ambiguous" => "ambiguous",
                "error" => "error",
                other => other,
            };
            ReadingStatusMatchResultDto {
                id: row.get("id"),
                series_id: row.get("series_id"),
                series_name: row.get::<Option<String>, _>("entity_name").unwrap_or_default(),
                status: status.to_string(),
                anilist_id: detail.as_ref().and_then(|d| d["anilist_id"].as_i64()).map(|v| v as i32),
                anilist_title: detail.as_ref().and_then(|d| d["anilist_title"].as_str().map(String::from)),
                anilist_url: detail.as_ref().and_then(|d| d["anilist_url"].as_str().map(String::from)),
                error_message: row.get("message"),
            }
        })
        .collect();

    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct ResultsQuery {
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

pub(crate) async fn process_reading_status_match(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(), String> {
    let (token, _, _) = anilist::load_anilist_settings(pool)
        .await
        .map_err(|e| e.message)?;

    let series_names: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT COALESCE(s.name, 'unclassified')
        FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE b.library_id = $1
        ORDER BY 1
        "#,
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let total = series_names.len() as i32;
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let already_linked: std::collections::HashSet<String> = sqlx::query_scalar(
        "SELECT s.name FROM anilist_series_links asl JOIN series s ON s.id = asl.series_id WHERE asl.library_id = $1",
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .collect();

    let mut processed = 0i32;

    for series_name in &series_names {
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
        .bind(series_name)
        .execute(pool)
        .await
        .ok();

        if series_name == "unclassified" {
            insert_event(pool, job_id, "anilist_already_linked", "info", Some(series_name), None, None).await;
            continue;
        }

        if already_linked.contains(series_name) {
            insert_event(pool, job_id, "anilist_already_linked", "info", Some(series_name), None, None).await;
            continue;
        }

        match search_and_link(pool, library_id, series_name, &token).await {
            Ok(Outcome::Linked { anilist_id, anilist_title, anilist_url }) => {
                insert_event(pool, job_id, "anilist_linked", "info", Some(series_name), None, Some(serde_json::json!({"anilist_id": anilist_id, "anilist_title": anilist_title, "anilist_url": anilist_url}))).await;
            }
            Ok(Outcome::NoResults) => {
                insert_event(pool, job_id, "anilist_no_results", "info", Some(series_name), None, None).await;
            }
            Ok(Outcome::Ambiguous) => {
                insert_event(pool, job_id, "anilist_ambiguous", "warning", Some(series_name), None, None).await;
            }
            Err(e) if e.contains("429") || e.contains("Too Many Requests") => {
                warn!("[READING_STATUS_MATCH] rate limit hit for '{series_name}', waiting 10s before retry");
                tokio::time::sleep(Duration::from_secs(10)).await;
                match search_and_link(pool, library_id, series_name, &token).await {
                    Ok(Outcome::Linked { anilist_id, anilist_title, anilist_url }) => {
                        insert_event(pool, job_id, "anilist_linked", "info", Some(series_name), None, Some(serde_json::json!({"anilist_id": anilist_id, "anilist_title": anilist_title, "anilist_url": anilist_url}))).await;
                    }
                    Ok(Outcome::NoResults) => {
                        insert_event(pool, job_id, "anilist_no_results", "info", Some(series_name), None, None).await;
                    }
                    Ok(Outcome::Ambiguous) => {
                        insert_event(pool, job_id, "anilist_ambiguous", "warning", Some(series_name), None, None).await;
                    }
                    Err(e2) => {
                        return Err(format!(
                            "AniList rate limit exceeded (429) — job stopped after {processed}/{total} series: {e2}"
                        ));
                    }
                }
            }
            Err(e) => {
                warn!("[READING_STATUS_MATCH] series '{series_name}': {e}");
                insert_event(pool, job_id, "error", "error", Some(series_name), Some(&e), None).await;
            }
        }

        // Respect AniList rate limit (~90 req/min)
        tokio::time::sleep(Duration::from_millis(700)).await;
    }

    // Build stats from events table
    let counts = sqlx::query(
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut count_linked = 0i64;
    let mut count_already_linked = 0i64;
    let mut count_no_results = 0i64;
    let mut count_ambiguous = 0i64;
    let mut count_errors = 0i64;
    for row in &counts {
        let s: String = row.get("event_type");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "anilist_linked" => count_linked = c,
            "anilist_already_linked" => count_already_linked = c,
            "anilist_no_results" => count_no_results = c,
            "anilist_ambiguous" => count_ambiguous = c,
            "error" => count_errors = c,
            _ => {}
        }
    }

    let stats = serde_json::json!({
        "total_series": total as i64,
        "linked": count_linked,
        "already_linked": count_already_linked,
        "no_results": count_no_results,
        "ambiguous": count_ambiguous,
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
        "[READING_STATUS_MATCH] job={job_id} completed: {}/{} series, linked={count_linked}, ambiguous={count_ambiguous}, no_results={count_no_results}, errors={count_errors}",
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
        notifications::NotificationEvent::ReadingStatusMatchCompleted {
            library_name,
            total_series: total,
            linked: count_linked as i32,
            already_linked: count_already_linked,
            no_results: count_no_results,
            ambiguous: count_ambiguous,
            errors: count_errors,
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

enum Outcome {
    Linked {
        anilist_id: i32,
        anilist_title: Option<String>,
        anilist_url: Option<String>,
    },
    NoResults,
    Ambiguous,
}

async fn search_and_link(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    token: &str,
) -> Result<Outcome, String> {
    let gql = r#"
        query SearchManga($search: String) {
            Page(perPage: 10) {
                media(search: $search, type: MANGA, sort: [SEARCH_MATCH]) {
                    id
                    title { romaji english native }
                    siteUrl
                }
            }
        }
    "#;

    let data = anilist::anilist_graphql(token, gql, serde_json::json!({ "search": series_name }))
        .await
        .map_err(|e| e.message)?;

    let media: Vec<serde_json::Value> = match data["Page"]["media"].as_array() {
        Some(arr) => arr.clone(),
        None => return Ok(Outcome::NoResults),
    };

    if media.is_empty() {
        return Ok(Outcome::NoResults);
    }

    let normalized_query = normalize_title(series_name);
    let exact_matches: Vec<_> = media
        .iter()
        .filter(|m| {
            let romaji = m["title"]["romaji"].as_str().map(normalize_title);
            let english = m["title"]["english"].as_str().map(normalize_title);
            let native = m["title"]["native"].as_str().map(normalize_title);
            romaji.as_deref() == Some(&normalized_query)
                || english.as_deref() == Some(&normalized_query)
                || native.as_deref() == Some(&normalized_query)
        })
        .collect();

    let candidate = if exact_matches.len() == 1 {
        exact_matches[0]
    } else if exact_matches.is_empty() && media.len() == 1 {
        &media[0]
    } else {
        return Ok(Outcome::Ambiguous);
    };

    let anilist_id = candidate["id"].as_i64().unwrap_or(0) as i32;
    let anilist_title = candidate["title"]["english"]
        .as_str()
        .or_else(|| candidate["title"]["romaji"].as_str())
        .map(String::from);
    let anilist_url = candidate["siteUrl"].as_str().map(String::from);

    let series_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM series WHERE library_id = $1 AND name = $2",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("series lookup failed for '{}': {}", series_name, e))?;

    sqlx::query(
        r#"
        INSERT INTO anilist_series_links (library_id, series_id, provider, anilist_id, anilist_title, anilist_url, status, linked_at)
        VALUES ($1, $2, 'anilist', $3, $4, $5, 'linked', NOW())
        ON CONFLICT (series_id, provider) DO NOTHING
        "#,
    )
    .bind(library_id)
    .bind(series_id)
    .bind(anilist_id)
    .bind(&anilist_title)
    .bind(&anilist_url)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(Outcome::Linked {
        anilist_id,
        anilist_title,
        anilist_url,
    })
}

fn normalize_title(s: &str) -> String {
    s.to_lowercase()
        .replace([':', '!', '?', '.', ',', '\'', '"', '-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn build_match_stats(pool: &PgPool, job_id: Uuid) -> serde_json::Value {
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

    let mut linked = 0i64;
    let mut already_linked = 0i64;
    let mut no_results = 0i64;
    let mut ambiguous = 0i64;
    let mut errors = 0i64;
    for row in &counts {
        let s: String = row.get("event_type");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "anilist_linked" => linked = c,
            "anilist_already_linked" => already_linked = c,
            "anilist_no_results" => no_results = c,
            "anilist_ambiguous" => ambiguous = c,
            "error" => errors = c,
            _ => {}
        }
    }

    serde_json::json!({
        "total_series": total.unwrap_or(0) as i64,
        "linked": linked,
        "already_linked": already_linked,
        "no_results": no_results,
        "ambiguous": ambiguous,
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
mod tests {
    use super::normalize_title;

    #[test]
    fn normalize_basic() {
        assert_eq!(normalize_title("Hello World"), "hello world");
    }

    #[test]
    fn normalize_removes_punctuation() {
        assert_eq!(normalize_title("One Piece: Stampede!"), "one piece stampede");
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize_title("  Naruto   Shippuden  "), "naruto shippuden");
    }

    #[test]
    fn normalize_replaces_special_chars() {
        assert_eq!(normalize_title("Dragon-Ball_Z"), "dragon ball z");
        assert_eq!(normalize_title("JoJo's Bizarre Adventure"), "jojo s bizarre adventure");
        assert_eq!(normalize_title("What...?!"), "what");
    }

    #[test]
    fn normalize_mixed_case() {
        assert_eq!(normalize_title("FULLMETAL ALCHEMIST"), "fullmetal alchemist");
        assert_eq!(normalize_title("FuLlMeTaL"), "fullmetal");
    }

    #[test]
    fn normalize_empty_string() {
        assert_eq!(normalize_title(""), "");
    }

    #[test]
    fn normalize_only_punctuation() {
        assert_eq!(normalize_title(":!?.,'-\"_"), "");
    }

    #[test]
    fn normalize_preserves_numbers() {
        assert_eq!(normalize_title("One Piece 100"), "one piece 100");
    }

    #[test]
    fn normalize_quotes_replaced() {
        assert_eq!(normalize_title("\"Quoted Title\""), "quoted title");
    }

    #[test]
    fn normalize_comma_in_title() {
        assert_eq!(normalize_title("Spy, Family"), "spy family");
    }

    #[test]
    fn normalize_idempotent() {
        let input = "already normalized";
        assert_eq!(normalize_title(input), input);
    }

    #[test]
    fn normalize_consecutive_special_chars() {
        assert_eq!(normalize_title("title---subtitle"), "title subtitle");
    }

    // -----------------------------------------------------------------------
    // insert_event tests
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn event_anilist_linked_written(pool: sqlx::PgPool) {
        use sqlx::Row;
        use uuid::Uuid;

        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestEvtLib', '/libraries/test_evt')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'running', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        super::insert_event(&pool, job_id, "anilist_linked", "info", Some("Naruto"), None, None).await;

        let row = sqlx::query(
            "SELECT event_type, level, entity_name, message, detail FROM index_job_events WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.get::<String, _>("event_type"), "anilist_linked");
        assert_eq!(row.get::<String, _>("level"), "info");
        assert_eq!(row.get::<Option<String>, _>("entity_name"), Some("Naruto".to_string()));
        assert!(row.get::<Option<String>, _>("message").is_none());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn event_error_has_error_level(pool: sqlx::PgPool) {
        use sqlx::Row;
        use uuid::Uuid;

        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestEvtErrLib', '/libraries/test_evt_err')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'running', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        super::insert_event(&pool, job_id, "error", "error", Some("BrokenSeries"), Some("AniList API failed"), None).await;

        let row = sqlx::query(
            "SELECT event_type, level, entity_name, message FROM index_job_events WHERE job_id = $1",
        )
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.get::<String, _>("event_type"), "error");
        assert_eq!(row.get::<String, _>("level"), "error");
        assert_eq!(row.get::<Option<String>, _>("entity_name"), Some("BrokenSeries".to_string()));
        assert_eq!(row.get::<Option<String>, _>("message"), Some("AniList API failed".to_string()));
    }

    /// Regression: series_id must be populated via LEFT JOIN series when the series exists.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_id_returned_when_series_exists(pool: sqlx::PgPool) {
        use sqlx::Row;
        use uuid::Uuid;

        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Naruto')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
             VALUES ($1, 'anilist_linked', 'info', 'Naruto')",
        )
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();

        // Run the actual query from get_match_results (no status filter)
        let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        let returned_series_id: Option<Uuid> = rows[0].get("series_id");
        assert_eq!(returned_series_id, Some(series_id), "series_id should match the created series");
    }

    /// Regression: series_id should be None when no matching series exists.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_id_none_when_series_missing(pool: sqlx::PgPool) {
        use sqlx::Row;
        use uuid::Uuid;

        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert event with a series_name that does NOT exist in series table
        sqlx::query(
            "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
             VALUES ($1, 'anilist_no_results', 'info', 'NonExistentSeries')",
        )
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();

        let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        let returned_series_id: Option<Uuid> = rows[0].get("series_id");
        assert!(returned_series_id.is_none(), "series_id should be None when series does not exist");
    }

    /// Regression: LEFT JOIN with LOWER(unaccent()) resolves series_id even when
    /// the event entity_name has different accents/casing than the series name.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_id_resolved_with_unaccent_in_results(pool: sqlx::PgPool) {
        use sqlx::Row;
        use uuid::Uuid;

        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestUnaccent', '/libraries/test_unaccent')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        // Series with accented name
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Astérix')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert event with entity_name WITHOUT accent
        sqlx::query(
            "INSERT INTO index_job_events (job_id, event_type, level, entity_name) \
             VALUES ($1, 'anilist_linked', 'info', 'Asterix')",
        )
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();

        // Run the actual query from get_match_results
        let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        let returned_series_id: Option<Uuid> = rows[0].get("series_id");
        assert_eq!(
            returned_series_id,
            Some(series_id),
            "series_id should be resolved despite accent difference (Astérix vs Asterix)"
        );
    }

    // -----------------------------------------------------------------------
    // Report endpoint: GROUP BY event_type counts from index_job_events
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn report_counts_from_events(pool: sqlx::PgPool) {
        use sqlx::Row;
        use uuid::Uuid;

        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'MatchReportLib', '/libraries/match_report')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at, total_files) VALUES ($1, $2, 'reading_status_match', 'success', NOW(), 7)",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert events with different event_types
        super::insert_event(&pool, job_id, "anilist_linked", "info", Some("Naruto"), None, Some(serde_json::json!({"anilist_id": 20}))).await;
        super::insert_event(&pool, job_id, "anilist_linked", "info", Some("Bleach"), None, Some(serde_json::json!({"anilist_id": 21}))).await;
        super::insert_event(&pool, job_id, "anilist_already_linked", "info", Some("OnePiece"), None, None).await;
        super::insert_event(&pool, job_id, "anilist_no_results", "info", Some("Obscure"), None, None).await;
        super::insert_event(&pool, job_id, "anilist_no_results", "info", Some("Obscure2"), None, None).await;
        super::insert_event(&pool, job_id, "anilist_ambiguous", "warning", Some("Dragon"), None, None).await;
        super::insert_event(&pool, job_id, "error", "error", Some("Broken"), Some("API timeout"), None).await;

        // Run the report query
        let counts = sqlx::query(
            "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
        )
        .bind(job_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        let mut linked = 0i64;
        let mut already_linked = 0i64;
        let mut no_results = 0i64;
        let mut ambiguous = 0i64;
        let mut errors = 0i64;

        for r in &counts {
            let event_type: String = r.get("event_type");
            let cnt: i64 = r.get("cnt");
            match event_type.as_str() {
                "anilist_linked" => linked = cnt,
                "anilist_already_linked" => already_linked = cnt,
                "anilist_no_results" => no_results = cnt,
                "anilist_ambiguous" => ambiguous = cnt,
                "error" => errors = cnt,
                _ => {}
            }
        }

        assert_eq!(linked, 2, "anilist_linked -> linked");
        assert_eq!(already_linked, 1, "anilist_already_linked -> already_linked");
        assert_eq!(no_results, 2, "anilist_no_results -> no_results");
        assert_eq!(ambiguous, 1, "anilist_ambiguous -> ambiguous");
        assert_eq!(errors, 1, "error -> errors");
    }

    // -----------------------------------------------------------------------
    // Results endpoint: event detail JSON mapped to ReadingStatusMatchResultDto
    // -----------------------------------------------------------------------

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn results_mapped_from_event_detail(pool: sqlx::PgPool) {
        use sqlx::Row;
        use uuid::Uuid;

        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'MatchResultLib', '/libraries/match_result')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'reading_status_match', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let detail = serde_json::json!({
            "anilist_id": 12345,
            "anilist_title": "Naruto Shippuden",
            "anilist_url": "https://anilist.co/manga/12345",
        });

        super::insert_event(&pool, job_id, "anilist_linked", "info", Some("Naruto"), None, Some(detail)).await;

        // Run the actual results query (no filter)
        let rows = sqlx::query(
            "SELECT e.id, e.entity_name, e.event_type, e.message, e.detail, s.id AS series_id
             FROM index_job_events e
             LEFT JOIN series s ON s.library_id = $2 AND LOWER(unaccent(s.name)) = LOWER(unaccent(e.entity_name))
             WHERE e.job_id = $1
             ORDER BY e.event_type, e.entity_name",
        )
        .bind(job_id)
        .bind(Some(library_id))
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);

        let row = &rows[0];
        let event_type: String = row.get("event_type");
        let row_detail: Option<serde_json::Value> = row.get("detail");

        // Map event_type to status like the endpoint does
        let status = match event_type.as_str() {
            "anilist_linked" => "linked",
            "anilist_already_linked" => "already_linked",
            "anilist_no_results" => "no_results",
            "anilist_ambiguous" => "ambiguous",
            "error" => "error",
            other => other,
        };

        assert_eq!(status, "linked");

        let d = row_detail.as_ref().unwrap();
        let anilist_id = d["anilist_id"].as_i64().map(|v| v as i32);
        let anilist_title = d["anilist_title"].as_str().map(String::from);
        let anilist_url = d["anilist_url"].as_str().map(String::from);

        assert_eq!(anilist_id, Some(12345));
        assert_eq!(anilist_title.as_deref(), Some("Naruto Shippuden"));
        assert_eq!(anilist_url.as_deref(), Some("https://anilist.co/manga/12345"));

        let entity_name: Option<String> = row.get("entity_name");
        assert_eq!(entity_name.as_deref(), Some("Naruto"));
    }
}
