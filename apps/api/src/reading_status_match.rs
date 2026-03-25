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
    pub library_id: String,
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
    let library_id: Uuid = body
        .library_id
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
        "SELECT status, COUNT(*) as cnt FROM reading_status_match_results WHERE job_id = $1 GROUP BY status",
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
        let status: String = r.get("status");
        let cnt: i64 = r.get("cnt");
        match status.as_str() {
            "linked" => linked = cnt,
            "already_linked" => already_linked = cnt,
            "no_results" => no_results = cnt,
            "ambiguous" => ambiguous = cnt,
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
    let rows = if let Some(status_filter) = &query.status {
        sqlx::query(
            "SELECT id, series_name, status, anilist_id, anilist_title, anilist_url, error_message
             FROM reading_status_match_results
             WHERE job_id = $1 AND status = $2
             ORDER BY series_name",
        )
        .bind(job_id)
        .bind(status_filter)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, series_name, status, anilist_id, anilist_title, anilist_url, error_message
             FROM reading_status_match_results
             WHERE job_id = $1
             ORDER BY status, series_name",
        )
        .bind(job_id)
        .fetch_all(&state.pool)
        .await?
    };

    let results = rows
        .iter()
        .map(|row| ReadingStatusMatchResultDto {
            id: row.get("id"),
            series_name: row.get("series_name"),
            status: row.get("status"),
            anilist_id: row.get("anilist_id"),
            anilist_title: row.get("anilist_title"),
            anilist_url: row.get("anilist_url"),
            error_message: row.get("error_message"),
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
        SELECT DISTINCT COALESCE(NULLIF(series, ''), 'unclassified')
        FROM books
        WHERE library_id = $1
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
        "SELECT series_name FROM anilist_series_links WHERE library_id = $1",
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
            insert_result(pool, job_id, library_id, series_name, "already_linked", None, None, None, None).await;
            continue;
        }

        if already_linked.contains(series_name) {
            insert_result(pool, job_id, library_id, series_name, "already_linked", None, None, None, None).await;
            continue;
        }

        match search_and_link(pool, library_id, series_name, &token).await {
            Ok(Outcome::Linked { anilist_id, anilist_title, anilist_url }) => {
                insert_result(pool, job_id, library_id, series_name, "linked", Some(anilist_id), anilist_title.as_deref(), anilist_url.as_deref(), None).await;
            }
            Ok(Outcome::NoResults) => {
                insert_result(pool, job_id, library_id, series_name, "no_results", None, None, None, None).await;
            }
            Ok(Outcome::Ambiguous) => {
                insert_result(pool, job_id, library_id, series_name, "ambiguous", None, None, None, None).await;
            }
            Err(e) if e.contains("429") || e.contains("Too Many Requests") => {
                warn!("[READING_STATUS_MATCH] rate limit hit for '{series_name}', waiting 10s before retry");
                tokio::time::sleep(Duration::from_secs(10)).await;
                match search_and_link(pool, library_id, series_name, &token).await {
                    Ok(Outcome::Linked { anilist_id, anilist_title, anilist_url }) => {
                        insert_result(pool, job_id, library_id, series_name, "linked", Some(anilist_id), anilist_title.as_deref(), anilist_url.as_deref(), None).await;
                    }
                    Ok(Outcome::NoResults) => {
                        insert_result(pool, job_id, library_id, series_name, "no_results", None, None, None, None).await;
                    }
                    Ok(Outcome::Ambiguous) => {
                        insert_result(pool, job_id, library_id, series_name, "ambiguous", None, None, None, None).await;
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
                insert_result(pool, job_id, library_id, series_name, "error", None, None, None, Some(&e)).await;
            }
        }

        // Respect AniList rate limit (~90 req/min)
        tokio::time::sleep(Duration::from_millis(700)).await;
    }

    // Build stats from results table
    let counts = sqlx::query(
        "SELECT status, COUNT(*) as cnt FROM reading_status_match_results WHERE job_id = $1 GROUP BY status",
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
        let s: String = row.get("status");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "linked" => count_linked = c,
            "already_linked" => count_already_linked = c,
            "no_results" => count_no_results = c,
            "ambiguous" => count_ambiguous = c,
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
        },
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn insert_result(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    status: &str,
    anilist_id: Option<i32>,
    anilist_title: Option<&str>,
    anilist_url: Option<&str>,
    error_message: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO reading_status_match_results
            (job_id, library_id, series_name, status, anilist_id, anilist_title, anilist_url, error_message)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(job_id)
    .bind(library_id)
    .bind(series_name)
    .bind(status)
    .bind(anilist_id)
    .bind(anilist_title)
    .bind(anilist_url)
    .bind(error_message)
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

    sqlx::query(
        r#"
        INSERT INTO anilist_series_links (library_id, series_name, provider, anilist_id, anilist_title, anilist_url, status, linked_at)
        VALUES ($1, $2, 'anilist', $3, $4, $5, 'linked', NOW())
        ON CONFLICT (library_id, series_name, provider) DO NOTHING
        "#,
    )
    .bind(library_id)
    .bind(series_name)
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
        "SELECT status, COUNT(*) as cnt FROM reading_status_match_results WHERE job_id = $1 GROUP BY status",
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
        let s: String = row.get("status");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "linked" => linked = c,
            "already_linked" => already_linked = c,
            "no_results" => no_results = c,
            "ambiguous" => ambiguous = c,
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
