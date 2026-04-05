use axum::{
    extract::{Path as AxumPath, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use utoipa::ToSchema;
use tracing::{info, warn};

use crate::{error::ApiError, job_helpers::{is_job_cancelled, update_progress, insert_event}, metadata_providers, state::AppState};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct MetadataBatchRequest {
    pub library_id: Option<String>,
    /// When true, delete existing metadata links and re-match all series.
    #[serde(default)]
    pub force_rematch: bool,
}

#[derive(Serialize, ToSchema)]
pub struct MetadataBatchReportDto {
    #[schema(value_type = String)]
    pub job_id: Uuid,
    pub status: String,
    pub total_series: i64,
    pub processed: i64,
    pub auto_matched: i64,
    pub no_results: i64,
    pub too_many_results: i64,
    pub low_confidence: i64,
    pub already_linked: i64,
    pub errors: i64,
}

#[derive(Serialize, ToSchema)]
pub struct MetadataBatchResultDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = Option<String>)]
    pub series_id: Option<Uuid>,
    pub series_name: String,
    pub status: String,
    pub provider_used: Option<String>,
    pub fallback_used: bool,
    pub candidates_count: i32,
    pub best_confidence: Option<f32>,
    pub best_candidate_json: Option<serde_json::Value>,
    #[schema(value_type = Option<String>)]
    pub link_id: Option<Uuid>,
    pub error_message: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchResultsQuery {
    pub status: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// POST /metadata/batch — Trigger a batch metadata job
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/batch",
    tag = "metadata",
    request_body = MetadataBatchRequest,
    responses(
        (status = 200, description = "Job created"),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_batch(
    State(state): State<AppState>,
    Json(body): Json<MetadataBatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // All libraries case
    if body.library_id.is_none() {
        let library_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM libraries WHERE metadata_provider IS DISTINCT FROM 'none' ORDER BY name"
        )
        .fetch_all(&state.pool)
        .await?;
        let mut last_job_id: Option<Uuid> = None;
        for library_id in library_ids {
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'metadata_batch' AND status IN ('pending', 'running') LIMIT 1",
            )
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?;
            if existing.is_some() { continue; }
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_batch', 'running', NOW())",
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
                if let Err(e) = process_metadata_batch(&pool, job_id, library_id).await {
                    warn!("[METADATA_BATCH] job {job_id} failed: {e}");
                    let _ = sqlx::query(
                        "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
                    )
                    .bind(job_id)
                    .bind(e.to_string())
                    .execute(&pool)
                    .await;
                    notifications::notify(
                        pool.clone(),
                        notifications::NotificationEvent::MetadataBatchFailed {
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

    // Verify library exists
    sqlx::query("SELECT 1 FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("library not found"))?;

    // Check library provider -- if "none", refuse batch
    let lib_row = sqlx::query("SELECT metadata_provider FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(&state.pool)
        .await?;
    let lib_provider: Option<String> = lib_row.get("metadata_provider");
    if lib_provider.as_deref() == Some("none") {
        return Err(ApiError::bad_request("This library has metadata disabled (provider set to 'none')"));
    }

    // Check no existing running metadata_batch job for this library
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id = $1 AND type IN ('metadata_batch', 'metadata_batch_rematch') AND status IN ('pending', 'running') LIMIT 1",
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

    let job_type = if body.force_rematch { "metadata_batch_rematch" } else { "metadata_batch" };
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, $3, 'running', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .bind(job_type)
    .execute(&state.pool)
    .await?;

    // Spawn the background processing task (status already 'running' to avoid poller race)
    let pool = state.pool.clone();
    let library_name: Option<String> = sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    tokio::spawn(async move {
        if let Err(e) = process_metadata_batch(&pool, job_id, library_id).await {
            warn!("[METADATA_BATCH] job {job_id} failed: {e}");
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .bind(e.to_string())
            .execute(&pool)
            .await;
            notifications::notify(
                pool.clone(),
                notifications::NotificationEvent::MetadataBatchFailed {
                    library_name,
                    error: e.to_string(),
                },
            );
        }
    });

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "pending",
    })))
}

// ---------------------------------------------------------------------------
// GET /metadata/batch/:id/report — Summary report
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/metadata/batch/{id}/report",
    tag = "metadata",
    params(("id" = String, Path, description = "Job UUID")),
    responses(
        (status = 200, body = MetadataBatchReportDto),
        (status = 404, description = "Job not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_batch_report(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<Uuid>,
) -> Result<Json<MetadataBatchReportDto>, ApiError> {
    let job = sqlx::query(
        "SELECT status, total_files, processed_files FROM index_jobs WHERE id = $1 AND type = 'metadata_batch'",
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("job not found"))?;

    let job_status: String = job.get("status");
    let total_series: Option<i32> = job.get("total_files");
    let processed: Option<i32> = job.get("processed_files");

    // Count by event_type
    let counts = sqlx::query(
        r#"
        SELECT event_type, COUNT(*) as cnt
        FROM index_job_events
        WHERE job_id = $1
        GROUP BY event_type
        "#,
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await?;

    let mut auto_matched: i64 = 0;
    let mut no_results: i64 = 0;
    let mut too_many_results: i64 = 0;
    let mut low_confidence: i64 = 0;
    let mut already_linked: i64 = 0;
    let mut errors: i64 = 0;

    for row in &counts {
        let event_type: String = row.get("event_type");
        let cnt: i64 = row.get("cnt");
        match event_type.as_str() {
            "metadata_matched" => auto_matched = cnt,
            "metadata_no_results" => no_results = cnt,
            "metadata_too_many" => too_many_results = cnt,
            "metadata_low_confidence" => low_confidence = cnt,
            "metadata_already_linked" => already_linked = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    Ok(Json(MetadataBatchReportDto {
        job_id,
        status: job_status,
        total_series: total_series.unwrap_or(0) as i64,
        processed: processed.unwrap_or(0) as i64,
        auto_matched,
        no_results,
        too_many_results,
        low_confidence,
        already_linked,
        errors,
    }))
}

// ---------------------------------------------------------------------------
// GET /metadata/batch/:id/results — Per-series results
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/metadata/batch/{id}/results",
    tag = "metadata",
    params(
        ("id" = String, Path, description = "Job UUID"),
        ("status" = Option<String>, Query, description = "Filter by status"),
        ("page" = Option<i64>, Query, description = "Page number (1-based)"),
        ("limit" = Option<i64>, Query, description = "Page size"),
    ),
    responses(
        (status = 200, body = Vec<MetadataBatchResultDto>),
    ),
    security(("Bearer" = []))
)]
pub async fn get_batch_results(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<Uuid>,
    Query(query): Query<BatchResultsQuery>,
) -> Result<Json<Vec<MetadataBatchResultDto>>, ApiError> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = (page - 1) * limit;

    // Map frontend status filter to event_type(s)
    let event_type_filter: Option<String> = query.status.as_deref().map(|s| match s {
        "auto_matched" => "metadata_matched".to_string(),
        "no_results" => "metadata_no_results".to_string(),
        "too_many_results" => "metadata_too_many".to_string(),
        "low_confidence" => "metadata_low_confidence".to_string(),
        "already_linked" => "metadata_already_linked".to_string(),
        "error" => "error".to_string(),
        other => other.to_string(),
    });

    // Get library_id from the job to resolve series_id
    let job_library_id: Option<Uuid> = sqlx::query_scalar("SELECT library_id FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();

    let rows = sqlx::query(
        r#"
        SELECT ije.id, ije.event_type, ije.entity_id, ije.entity_name, ije.message, ije.detail,
               s.id AS series_id
        FROM index_job_events ije
        LEFT JOIN series s ON s.library_id = $5 AND LOWER(unaccent(s.name)) = LOWER(unaccent(ije.entity_name))
        WHERE ije.job_id = $1
          AND (ije.event_type LIKE 'metadata_%' OR ije.event_type = 'error')
          AND ($2::text IS NULL OR ije.event_type = $2)
        ORDER BY
            CASE ije.event_type
                WHEN 'metadata_matched' THEN 1
                WHEN 'metadata_low_confidence' THEN 2
                WHEN 'metadata_too_many' THEN 3
                WHEN 'metadata_no_results' THEN 4
                WHEN 'error' THEN 5
                WHEN 'metadata_already_linked' THEN 6
                ELSE 7
            END,
            ije.entity_name ASC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(job_id)
    .bind(event_type_filter.as_deref())
    .bind(limit)
    .bind(offset)
    .bind(job_library_id)
    .fetch_all(&state.pool)
    .await?;

    let results: Vec<MetadataBatchResultDto> = rows
        .iter()
        .map(|row| {
            let event_type: String = row.get("event_type");
            let detail: Option<serde_json::Value> = row.get("detail");

            // Map event_type back to the status the frontend expects
            let status = match event_type.as_str() {
                "metadata_matched" => "auto_matched",
                "metadata_no_results" => "no_results",
                "metadata_too_many" => "too_many_results",
                "metadata_low_confidence" => "low_confidence",
                "metadata_already_linked" => "already_linked",
                "error" => "error",
                other => other,
            };

            let provider_used = detail.as_ref()
                .and_then(|d| d.get("provider"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let fallback_used = detail.as_ref()
                .and_then(|d| d.get("fallback_used"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let candidates_count = detail.as_ref()
                .and_then(|d| d.get("candidates_count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let best_confidence = detail.as_ref()
                .and_then(|d| d.get("confidence"))
                .and_then(|v| v.as_f64())
                .map(|f| f as f32);
            let best_candidate_json = detail.as_ref()
                .and_then(|d| d.get("best_candidate"))
                .cloned();
            let link_id = detail.as_ref()
                .and_then(|d| d.get("link_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<Uuid>().ok());
            let error_message: Option<String> = row.get("message");

            MetadataBatchResultDto {
                id: row.get("id"),
                series_id: row.get("series_id"),
                series_name: row.get::<Option<String>, _>("entity_name").unwrap_or_default(),
                status: status.to_string(),
                provider_used,
                fallback_used,
                candidates_count,
                best_confidence,
                best_candidate_json,
                link_id,
                error_message,
            }
        })
        .collect();

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

pub(crate) async fn process_metadata_batch(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(), String> {
    // Detect if this is a force-rematch job
    let job_type: String = sqlx::query_scalar("SELECT type FROM index_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    let force_rematch = job_type == "metadata_batch_rematch";

    // Set job to running
    sqlx::query("UPDATE index_jobs SET status = 'running', started_at = NOW() WHERE id = $1")
        .bind(job_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Get library provider config
    let lib_row = sqlx::query(
        "SELECT metadata_provider, fallback_metadata_provider FROM libraries WHERE id = $1",
    )
    .bind(library_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let primary_provider_name: Option<String> = lib_row.get("metadata_provider");
    let fallback_provider_name: Option<String> = lib_row.get("fallback_metadata_provider");

    // Resolve primary provider: library -> global setting -> google_books
    let primary_name = super::config::resolve_provider_name(pool, primary_provider_name.as_deref()).await;
    let fallback_name = fallback_provider_name
        .as_deref()
        .filter(|s| !s.is_empty() && *s != primary_name)
        .map(|s| s.to_string());

    info!(
        "[METADATA_BATCH] job={job_id} library={library_id} primary={primary_name} fallback={fallback_name:?}"
    );

    // Get all distinct series names for this library, least recently updated first
    let series_names: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT COALESCE(s.name, 'unclassified')
        FROM series s
        JOIN books b ON b.series_id = s.id
        WHERE b.library_id = $1
        GROUP BY s.id, s.name
        ORDER BY s.updated_at ASC NULLS FIRST
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

    // Get series that already have an approved link -- map name -> provider
    let linked_rows = sqlx::query(
        "SELECT s.name, eml.provider FROM external_metadata_links eml JOIN series s ON s.id = eml.series_id WHERE eml.library_id = $1 AND eml.status = 'approved'",
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let already_linked: std::collections::HashMap<String, String> = linked_rows
        .into_iter()
        .map(|row| (row.get::<String, _>("name"), row.get::<String, _>("provider")))
        .collect();

    let mut processed = 0i32;

    for series_name in &series_names {
        // Check cancellation
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

        // Skip unclassified
        if series_name == "unclassified" {
            processed += 1;
            update_progress(pool, job_id, processed, total, series_name).await;
            insert_event(pool, job_id, "metadata_already_linked", "info", Some("series"), Some(series_name), Some("Unclassified series skipped"), None).await;
            continue;
        }

        // Skip already linked:
        // - Normal mode: skip any linked series
        // - Rematch mode: skip only if already linked to the TARGET provider (no point re-matching)
        let linked_provider = already_linked.get(series_name.as_str());
        let should_skip = if force_rematch {
            linked_provider.map(|p| p == &primary_name).unwrap_or(false)
        } else {
            linked_provider.is_some()
        };
        if should_skip {
            processed += 1;
            update_progress(pool, job_id, processed, total, series_name).await;
            insert_event(pool, job_id, "metadata_already_linked", "info", Some("series"), Some(series_name), None, None).await;
            continue;
        }

        // Search with primary provider
        let (result_status, provider_used, fallback_used, candidates_count, best_confidence, best_candidate, link_id, error_msg) =
            match search_and_evaluate(pool, library_id, series_name, &primary_name, true).await {
                SearchOutcome::AutoMatch(candidate) => {
                    // Create link + approve + sync
                    match auto_apply(pool, library_id, series_name, &primary_name, &candidate).await {
                        Ok(lid) => (
                            "auto_matched",
                            Some(primary_name.clone()),
                            false,
                            1,
                            Some(candidate.confidence),
                            Some(serde_json::json!({
                                "title": candidate.title,
                                "external_id": candidate.external_id,
                            })),
                            Some(lid),
                            None,
                        ),
                        Err(e) => (
                            "error",
                            Some(primary_name.clone()),
                            false,
                            1,
                            Some(candidate.confidence),
                            None,
                            None,
                            Some(format!("Auto-apply failed: {e}")),
                        ),
                    }
                }
                SearchOutcome::NoResults => {
                    // Try fallback
                    if let Some(ref fb_name) = fallback_name {
                        match search_and_evaluate(pool, library_id, series_name, fb_name, true).await {
                            SearchOutcome::AutoMatch(candidate) => {
                                match auto_apply(pool, library_id, series_name, fb_name, &candidate).await {
                                    Ok(lid) => (
                                        "auto_matched",
                                        Some(fb_name.clone()),
                                        true,
                                        1,
                                        Some(candidate.confidence),
                                        Some(serde_json::json!({
                                            "title": candidate.title,
                                            "external_id": candidate.external_id,
                                        })),
                                        Some(lid),
                                        None,
                                    ),
                                    Err(e) => (
                                        "error",
                                        Some(fb_name.clone()),
                                        true,
                                        1,
                                        Some(candidate.confidence),
                                        None,
                                        None,
                                        Some(format!("Auto-apply failed: {e}")),
                                    ),
                                }
                            }
                            SearchOutcome::NoResults => (
                                "no_results",
                                Some(fb_name.clone()),
                                true,
                                0,
                                None,
                                None,
                                None,
                                Some("No results from primary or fallback provider".to_string()),
                            ),
                            SearchOutcome::TooManyResults(count, best) => (
                                "too_many_results",
                                Some(fb_name.clone()),
                                true,
                                count,
                                best.as_ref().map(|c| c.confidence),
                                best.map(|c| serde_json::json!({
                                    "title": c.title,
                                    "external_id": c.external_id,
                                    "external_url": c.external_url,
                                    "authors": c.authors,
                                    "description": c.description,
                                    "cover_url": c.cover_url,
                                    "total_volumes": c.total_volumes,
                                    "start_year": c.start_year,
                                    "confidence": c.confidence,
                                })),
                                None,
                                Some(format!("{count} results, manual review needed")),
                            ),
                            SearchOutcome::LowConfidence(candidate) => (
                                "low_confidence",
                                Some(fb_name.clone()),
                                true,
                                1,
                                Some(candidate.confidence),
                                Some(serde_json::json!({
                                    "title": candidate.title,
                                    "external_id": candidate.external_id,
                                    "external_url": candidate.external_url,
                                    "authors": candidate.authors,
                                    "description": candidate.description,
                                    "cover_url": candidate.cover_url,
                                    "total_volumes": candidate.total_volumes,
                                    "start_year": candidate.start_year,
                                    "confidence": candidate.confidence,
                                })),
                                None,
                                Some(format!("Best confidence: {:.0}%", candidate.confidence * 100.0)),
                            ),
                            SearchOutcome::Error(e) => (
                                "error",
                                Some(fb_name.clone()),
                                true,
                                0,
                                None,
                                None,
                                None,
                                Some(e),
                            ),
                        }
                    } else {
                        (
                            "no_results",
                            Some(primary_name.clone()),
                            false,
                            0,
                            None,
                            None,
                            None,
                            Some("No results found".to_string()),
                        )
                    }
                }
                SearchOutcome::TooManyResults(count, best) => (
                    "too_many_results",
                    Some(primary_name.clone()),
                    false,
                    count,
                    best.as_ref().map(|c| c.confidence),
                    best.map(|c| serde_json::json!({
                                    "title": c.title,
                                    "external_id": c.external_id,
                                    "external_url": c.external_url,
                                    "authors": c.authors,
                                    "description": c.description,
                                    "cover_url": c.cover_url,
                                    "total_volumes": c.total_volumes,
                                    "start_year": c.start_year,
                                    "confidence": c.confidence,
                                })),
                    None,
                    Some(format!("{count} results, manual review needed")),
                ),
                SearchOutcome::LowConfidence(candidate) => (
                    "low_confidence",
                    Some(primary_name.clone()),
                    false,
                    1,
                    Some(candidate.confidence),
                    Some(serde_json::json!({
                                    "title": candidate.title,
                                    "external_id": candidate.external_id,
                                    "external_url": candidate.external_url,
                                    "authors": candidate.authors,
                                    "description": candidate.description,
                                    "cover_url": candidate.cover_url,
                                    "total_volumes": candidate.total_volumes,
                                    "start_year": candidate.start_year,
                                    "confidence": candidate.confidence,
                                })),
                    None,
                    Some(format!("Best confidence: {:.0}%", candidate.confidence * 100.0)),
                ),
                SearchOutcome::Error(e) => (
                    "error",
                    Some(primary_name.clone()),
                    false,
                    0,
                    None,
                    None,
                    None,
                    Some(e),
                ),
            };

        // Insert event tracking (replaces insert_result -- events are the single source of truth)
        match result_status {
            "auto_matched" => {
                insert_event(
                    pool, job_id, "metadata_matched", "info", Some("series"), Some(series_name), None,
                    Some(serde_json::json!({
                        "provider": provider_used,
                        "fallback_used": fallback_used,
                        "candidates_count": candidates_count,
                        "confidence": best_confidence,
                        "best_candidate": best_candidate,
                        "link_id": link_id.map(|id| id.to_string()),
                    })),
                ).await;
            }
            "no_results" => {
                insert_event(
                    pool, job_id, "metadata_no_results", "info", Some("series"), Some(series_name),
                    error_msg.as_deref(),
                    Some(serde_json::json!({
                        "provider": provider_used,
                        "fallback_used": fallback_used,
                        "candidates_count": candidates_count,
                    })),
                ).await;
            }
            "low_confidence" => {
                insert_event(
                    pool, job_id, "metadata_low_confidence", "warning", Some("series"), Some(series_name),
                    error_msg.as_deref(),
                    Some(serde_json::json!({
                        "provider": provider_used,
                        "fallback_used": fallback_used,
                        "candidates_count": candidates_count,
                        "confidence": best_confidence,
                        "best_candidate": best_candidate,
                    })),
                ).await;
            }
            "too_many_results" => {
                insert_event(
                    pool, job_id, "metadata_too_many", "warning", Some("series"), Some(series_name),
                    error_msg.as_deref(),
                    Some(serde_json::json!({
                        "provider": provider_used,
                        "fallback_used": fallback_used,
                        "candidates_count": candidates_count,
                        "confidence": best_confidence,
                        "best_candidate": best_candidate,
                    })),
                ).await;
            }
            "error" => {
                insert_event(
                    pool, job_id, "error", "error", Some("series"), Some(series_name), error_msg.as_deref(),
                    Some(serde_json::json!({
                        "provider": provider_used,
                        "fallback_used": fallback_used,
                        "candidates_count": candidates_count,
                    })),
                ).await;
            }
            _ => {}
        }

        // In rematch mode, if we created a new link, delete old links from other providers
        if force_rematch && result_status == "auto_matched" {
            if let Some(new_link_id) = link_id {
                let _ = sqlx::query(
                    "DELETE FROM external_metadata_links WHERE library_id = $1 AND id != $2 AND series_id = (\
                     SELECT series_id FROM external_metadata_links WHERE id = $2)",
                )
                .bind(library_id)
                .bind(new_link_id)
                .execute(pool)
                .await;
            }
        }

        processed += 1;
        update_progress(pool, job_id, processed, total, series_name).await;

        // Rate limit: 1s delay between provider calls to avoid being blocked
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    // Build stats summary
    let stats = serde_json::json!({
        "total_series": total,
        "processed": processed,
    });

    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, stats_json = $2 WHERE id = $1",
    )
    .bind(job_id)
    .bind(stats)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    info!("[METADATA_BATCH] job={job_id} completed: {processed}/{total} series processed");

    // Count per-outcome stats from events for the notification
    let event_counts = sqlx::query(
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut n_auto_matched = 0i64;
    let mut n_no_results = 0i64;
    let mut n_low_confidence = 0i64;
    let mut n_too_many = 0i64;
    let mut n_already_linked = 0i64;
    let mut n_errors = 0i64;
    for row in &event_counts {
        let s: String = row.get("event_type");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "metadata_matched" => n_auto_matched = c,
            "metadata_no_results" => n_no_results = c,
            "metadata_low_confidence" => n_low_confidence = c,
            "metadata_too_many" => n_too_many = c,
            "metadata_already_linked" => n_already_linked = c,
            "error" => n_errors = c,
            _ => {}
        }
    }

    let library_name: Option<String> = sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    notifications::notify(
        pool.clone(),
        notifications::NotificationEvent::MetadataBatchCompleted {
            library_name,
            total_series: total,
            processed,
            auto_matched: n_auto_matched,
            no_results: n_no_results,
            low_confidence: n_low_confidence,
            too_many: n_too_many,
            already_linked: n_already_linked,
            errors: n_errors,
        },
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Search evaluation
// ---------------------------------------------------------------------------

enum SearchOutcome {
    AutoMatch(metadata_providers::SeriesCandidate),
    NoResults,
    TooManyResults(i32, Option<metadata_providers::SeriesCandidate>),
    LowConfidence(metadata_providers::SeriesCandidate),
    Error(String),
}

async fn search_and_evaluate(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    detailed: bool,
) -> SearchOutcome {
    let provider = match metadata_providers::get_provider(provider_name) {
        Some(p) => p,
        None => return SearchOutcome::Error(format!("Unknown provider: {provider_name}")),
    };

    let mut config = super::config::load_provider_config(pool, provider_name).await;
    config.detailed = detailed;

    let mut candidates = match provider.search_series(series_name, &config).await {
        Ok(c) => c,
        Err(e) => return SearchOutcome::Error(e),
    };

    if candidates.is_empty() {
        return SearchOutcome::NoResults;
    }

    // Boost confidence based on local book count vs candidate total_volumes
    let local_count: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books b \
         JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 AND LOWER(unaccent(s.name)) = LOWER(unaccent($2))",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_one(pool)
    .await
    .ok();

    if let Some(count) = local_count {
        super::config::boost_confidence_by_book_count(&mut candidates, count);
    }

    // Only auto-match if exactly 1 result with confidence == 1.0
    if candidates.len() == 1 && (candidates[0].confidence - 1.0).abs() < f32::EPSILON {
        return SearchOutcome::AutoMatch(candidates.into_iter().next().unwrap());
    }

    // Check if best candidate has perfect confidence
    let best = candidates.into_iter().next().unwrap();
    if (best.confidence - 1.0).abs() < f32::EPSILON {
        return SearchOutcome::AutoMatch(best);
    }

    if best.confidence < 1.0 {
        return SearchOutcome::LowConfidence(best);
    }

    SearchOutcome::TooManyResults(0, None)
}

async fn auto_apply(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<Uuid, String> {
    // Resolve series_id from series name
    let series_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM series WHERE library_id = $1 AND name = $2",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Series '{}' not found in library", series_name))?;

    // Create the external_metadata_link
    let metadata_json = &candidate.metadata_json;
    let row = sqlx::query(
        r#"
        INSERT INTO external_metadata_links
            (library_id, series_id, provider, external_id, external_url, status, confidence, metadata_json, total_volumes_external)
        VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8)
        ON CONFLICT (series_id, provider)
        DO UPDATE SET
            external_id = EXCLUDED.external_id,
            external_url = EXCLUDED.external_url,
            status = 'approved',
            confidence = EXCLUDED.confidence,
            metadata_json = EXCLUDED.metadata_json,
            total_volumes_external = EXCLUDED.total_volumes_external,
            matched_at = NOW(),
            approved_at = NOW(),
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(library_id)
    .bind(series_id)
    .bind(provider_name)
    .bind(&candidate.external_id)
    .bind(&candidate.external_url)
    .bind(candidate.confidence)
    .bind(metadata_json)
    .bind(candidate.total_volumes)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let link_id: Uuid = row.get("id");

    // Sync series metadata
    sync_series_from_candidate(pool, library_id, series_name, candidate).await?;

    // Sync books
    sync_books_from_provider(pool, link_id, library_id, series_name, provider_name, &candidate.external_id).await?;

    Ok(link_id)
}

/// Sync series metadata from a candidate (simplified version for batch use)
async fn sync_series_from_candidate(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<(), String> {
    let description = candidate.metadata_json
        .get("description")
        .and_then(|d| d.as_str())
        .or(candidate.description.as_deref());
    let authors = &candidate.authors;
    let publishers = &candidate.publishers;
    let start_year = candidate.start_year;
    let total_volumes = candidate.total_volumes;
    let status = if let Some(raw) = candidate.metadata_json.get("status").and_then(|s| s.as_str()) {
        Some(super::handlers::normalize_series_status(pool, raw).await)
    } else {
        None
    };
    let status = status.as_deref();

    sqlx::query(
        r#"
        INSERT INTO series (id, library_id, name, description, publishers, start_year, total_volumes, status, authors, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
        ON CONFLICT (library_id, name)
        DO UPDATE SET
            description = CASE
                WHEN (series.locked_fields->>'description')::boolean IS TRUE THEN series.description
                ELSE COALESCE(NULLIF(EXCLUDED.description, ''), series.description)
            END,
            publishers = CASE
                WHEN (series.locked_fields->>'publishers')::boolean IS TRUE THEN series.publishers
                WHEN array_length(EXCLUDED.publishers, 1) > 0 THEN EXCLUDED.publishers
                ELSE series.publishers
            END,
            start_year = CASE
                WHEN (series.locked_fields->>'start_year')::boolean IS TRUE THEN series.start_year
                ELSE COALESCE(EXCLUDED.start_year, series.start_year)
            END,
            total_volumes = CASE
                WHEN (series.locked_fields->>'total_volumes')::boolean IS TRUE THEN series.total_volumes
                ELSE COALESCE(EXCLUDED.total_volumes, series.total_volumes)
            END,
            status = CASE
                WHEN (series.locked_fields->>'status')::boolean IS TRUE THEN series.status
                ELSE COALESCE(EXCLUDED.status, series.status)
            END,
            authors = CASE
                WHEN (series.locked_fields->>'authors')::boolean IS TRUE THEN series.authors
                WHEN array_length(EXCLUDED.authors, 1) > 0 THEN EXCLUDED.authors
                ELSE series.authors
            END,
            updated_at = NOW()
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .bind(description)
    .bind(publishers)
    .bind(start_year)
    .bind(total_volumes)
    .bind(status)
    .bind(authors)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Sync books from provider (simplified for batch use)
async fn sync_books_from_provider(
    pool: &PgPool,
    link_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    external_id: &str,
) -> Result<(), String> {
    let provider = metadata_providers::get_provider(provider_name)
        .ok_or_else(|| format!("Unknown provider: {provider_name}"))?;

    let config = super::config::load_provider_config(pool, provider_name).await;

    let books = provider
        .get_series_books(external_id, &config)
        .await
        .map_err(|e| format!("provider books error: {e}"))?;

    // Delete existing book metadata for this link
    sqlx::query("DELETE FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Pre-fetch local books
    let local_books: Vec<(Uuid, Option<i32>, String)> = sqlx::query_as(
        r#"
        SELECT b.id, b.volume, b.title FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE b.library_id = $1
          AND COALESCE(s.name, 'unclassified') = $2
        ORDER BY b.volume NULLS LAST,
                 REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                 COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                 b.title ASC
        "#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let local_books_with_pos: Vec<(Uuid, i32, String)> = local_books
        .iter()
        .enumerate()
        .map(|(idx, (id, vol, title))| (*id, vol.unwrap_or((idx + 1) as i32), title.clone()))
        .collect();

    let mut matched_local_ids = std::collections::HashSet::new();

    for (ext_idx, book) in books.iter().enumerate() {
        let ext_vol = book.volume_number.unwrap_or((ext_idx + 1) as i32);

        // Match by volume number
        let mut local_book_id: Option<Uuid> = local_books_with_pos
            .iter()
            .find(|(id, v, _)| *v == ext_vol && !matched_local_ids.contains(id))
            .map(|(id, _, _)| *id);

        // Match by title containment
        if local_book_id.is_none() {
            let ext_title_lower = book.title.to_lowercase();
            local_book_id = local_books_with_pos
                .iter()
                .find(|(id, _, local_title)| {
                    if matched_local_ids.contains(id) {
                        return false;
                    }
                    let local_lower = local_title.to_lowercase();
                    local_lower.contains(&ext_title_lower) || ext_title_lower.contains(&local_lower)
                })
                .map(|(id, _, _)| *id);
        }

        if let Some(id) = local_book_id {
            matched_local_ids.insert(id);
        }

        sqlx::query(
            r#"
            INSERT INTO external_book_metadata
                (link_id, book_id, external_book_id, volume_number, title, authors, isbn, summary, cover_url, page_count, language, publish_date, metadata_json)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(link_id)
        .bind(local_book_id)
        .bind(&book.external_book_id)
        .bind(book.volume_number)
        .bind(&book.title)
        .bind(&book.authors)
        .bind(&book.isbn)
        .bind(&book.summary)
        .bind(&book.cover_url)
        .bind(book.page_count)
        .bind(&book.language)
        .bind(&book.publish_date)
        .bind(&book.metadata_json)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        // Push metadata to matched local book
        if let Some(book_id) = local_book_id {
            sqlx::query(
                r#"
                UPDATE books SET
                    summary = CASE
                        WHEN (locked_fields->>'summary')::boolean IS TRUE THEN summary
                        ELSE COALESCE(NULLIF($2, ''), summary)
                    END,
                    isbn = CASE
                        WHEN (locked_fields->>'isbn')::boolean IS TRUE THEN isbn
                        ELSE COALESCE(NULLIF($3, ''), isbn)
                    END,
                    publish_date = CASE
                        WHEN (locked_fields->>'publish_date')::boolean IS TRUE THEN publish_date
                        ELSE COALESCE(NULLIF($4, ''), publish_date)
                    END,
                    language = CASE
                        WHEN (locked_fields->>'language')::boolean IS TRUE THEN language
                        ELSE COALESCE(NULLIF($5, ''), language)
                    END,
                    authors = CASE
                        WHEN (locked_fields->>'authors')::boolean IS TRUE THEN authors
                        WHEN CARDINALITY($6::text[]) > 0 THEN $6
                        ELSE authors
                    END,
                    author = CASE
                        WHEN (locked_fields->>'authors')::boolean IS TRUE THEN author
                        WHEN CARDINALITY($6::text[]) > 0 THEN $6[1]
                        ELSE author
                    END,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(book_id)
            .bind(&book.summary)
            .bind(&book.isbn)
            .bind(&book.publish_date)
            .bind(&book.language)
            .bind(&book.authors)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    // Update synced_at on the link
    sqlx::query("UPDATE external_metadata_links SET synced_at = NOW(), updated_at = NOW() WHERE id = $1")
        .bind(link_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// Helpers moved to crate::job_helpers and super::config

#[cfg(test)]
#[path = "batch_tests.rs"]
mod batch_tests;
