use axum::{
    extract::{Path as AxumPath, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::jobs::lifecycle::{complete_job, fail_job, job_in_flight, JobScope};
use crate::metadata_providers;
use crate::metadata_providers::senscritique::RATE_LIMITED_ERROR;
use crate::{
    error::ApiError,
    job_helpers::{insert_event, is_job_cancelled, update_progress},
    state::AppState,
};

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
    /// Outcome of the secondary provider when it was evaluated, even if the
    /// primary outcome is the one reported for the series (#150).
    pub secondary: Option<MetadataProviderOutcomeDto>,
}

/// Outcome of one metadata provider for a batch result.
///
/// Mirrors the `detail` payload persisted in the job event so the backoffice
/// can show which provider matched and how the other one fared.
#[derive(Serialize, ToSchema)]
pub struct MetadataProviderOutcomeDto {
    pub status: String,
    pub provider: String,
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
            let existing = job_in_flight(
                &state.pool,
                JobScope::Library(library_id),
                &["metadata_batch"],
            )
            .await?;
            if existing.is_some() {
                continue;
            }
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_batch', 'running', NOW())",
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
                if let Err(e) = process_metadata_batch(&pool, job_id, library_id).await {
                    warn!("[METADATA_BATCH] job {job_id} failed: {e}");
                    let _ = fail_job(&pool, job_id, &e.to_string(), None).await;
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
        return Err(ApiError::bad_request(
            "This library has metadata disabled (provider set to 'none')",
        ));
    }

    // Check no existing running metadata_batch job for this library
    let existing = job_in_flight(
        &state.pool,
        JobScope::Library(library_id),
        &["metadata_batch", "metadata_batch_rematch"],
    )
    .await?;

    if let Some(existing_id) = existing {
        return Ok(Json(serde_json::json!({
            "id": existing_id.to_string(),
            "status": "already_running",
        })));
    }

    let job_type = if body.force_rematch {
        "metadata_batch_rematch"
    } else {
        "metadata_batch"
    };
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
    let library_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
    tokio::spawn(async move {
        if let Err(e) = process_metadata_batch(&pool, job_id, library_id).await {
            warn!("[METADATA_BATCH] job {job_id} failed: {e}");
            let _ = fail_job(&pool, job_id, &e.to_string(), None).await;
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
    let job_library_id: Option<Uuid> =
        sqlx::query_scalar("SELECT library_id FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();

    let rows = sqlx::query(
        r#"
        SELECT ije.id, ije.event_type, ije.entity_id, ije.entity_name, ije.message, ije.detail,
               s.id AS series_id
        FROM index_job_events ije
        LEFT JOIN series s ON s.library_id = $5 AND norm_text(s.name) = norm_text(ije.entity_name)
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

            let provider_used = detail
                .as_ref()
                .and_then(|d| d.get("provider"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let fallback_used = detail
                .as_ref()
                .and_then(|d| d.get("fallback_used"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let candidates_count = detail
                .as_ref()
                .and_then(|d| d.get("candidates_count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let best_confidence = detail
                .as_ref()
                .and_then(|d| d.get("confidence"))
                .and_then(|v| v.as_f64())
                .map(|f| f as f32);
            let best_candidate_json = detail
                .as_ref()
                .and_then(|d| d.get("best_candidate"))
                .cloned();
            let link_id = detail
                .as_ref()
                .and_then(|d| d.get("link_id"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<Uuid>().ok());
            let error_message: Option<String> = row.get("message");
            let secondary = detail
                .as_ref()
                .and_then(|d| d.get("secondary"))
                .and_then(provider_outcome_from_detail);

            MetadataBatchResultDto {
                id: row.get("id"),
                series_id: row.get("series_id"),
                series_name: row
                    .get::<Option<String>, _>("entity_name")
                    .unwrap_or_default(),
                status: status.to_string(),
                provider_used,
                fallback_used,
                candidates_count,
                best_confidence,
                best_candidate_json,
                link_id,
                error_message,
                secondary,
            }
        })
        .collect();

    Ok(Json(results))
}

/// Parse a provider outcome nested in an event `detail` payload.
pub(super) fn provider_outcome_from_detail(
    detail: &serde_json::Value,
) -> Option<MetadataProviderOutcomeDto> {
    let obj = detail.as_object()?;
    // `status` is what distinguishes a provider outcome from the small
    // `already_linked` payload (which only carries the linked flags).
    let status = obj.get("status")?.as_str()?;

    Some(MetadataProviderOutcomeDto {
        status: status.to_string(),
        provider: obj
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        fallback_used: obj
            .get("fallback_used")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        candidates_count: obj
            .get("candidates_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32,
        best_confidence: obj
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32),
        best_candidate_json: obj.get("best_candidate").cloned(),
        link_id: obj
            .get("link_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok()),
        error_message: obj
            .get("message")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
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
    let secondary_provider_name: Option<String> = lib_row.get("fallback_metadata_provider");

    // Resolve primary provider: library -> global setting -> google_books.
    // Canonicalize it (discovery aliases -> real provider) so the per-provider
    // link check below compares the same name that `search_and_evaluate` uses.
    let primary_name = metadata_providers::canonical_provider_name(
        &super::config::resolve_provider_name(pool, primary_provider_name.as_deref()).await,
    )
    .to_string();
    let secondary_name = secondary_provider_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(metadata_providers::canonical_provider_name)
        .map(str::to_string)
        .filter(|s| *s != primary_name)
        .and_then(|name| match metadata_providers::resolve_provider(&name) {
            Ok(_) => Some(name),
            Err(unknown) => {
                // Now that the secondary is queried for every eligible series,
                // an unknown name would turn the whole job red one series at a
                // time. Warn once and run without it instead.
                warn!(
                    "[METADATA_BATCH] job={job_id} ignoring unknown secondary provider '{unknown}'"
                );
                None
            }
        });

    info!(
        "[METADATA_BATCH] job={job_id} library={library_id} primary={primary_name} secondary={secondary_name:?}"
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

    // Approved links grouped per series and provider: each provider is
    // evaluated on its own, so a secondary link never prevents matching the
    // primary (and vice versa, #150).
    let approved_links = load_approved_links(pool, library_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut processed = 0i32;
    let mut sc_rate_limited = false;

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
            insert_event(
                pool,
                job_id,
                "metadata_already_linked",
                "info",
                Some("series"),
                Some(series_name),
                Some("Unclassified series skipped"),
                None,
            )
            .await;
            continue;
        }

        // Each configured provider is evaluated independently: a series may
        // already have the primary link but still miss the secondary one, and
        // vice versa. Previously any approved link skipped the whole series,
        // which prevented secondaries from ever being auto-matched (#150).
        let primary_linked = has_approved_link(&approved_links, series_name, &primary_name);
        let secondary_linked = secondary_name
            .as_deref()
            .is_some_and(|p| has_approved_link(&approved_links, series_name, p));

        // Circuit breaker: skip only the SensCritique lookup, not the whole
        // series, so the other provider can still be matched.
        let primary_is_sc = primary_name == "senscritique";
        let secondary_is_sc = secondary_name.as_deref() == Some("senscritique");
        let primary_skipped_sc = !primary_linked && sc_rate_limited && primary_is_sc;
        let secondary_skipped_sc = !secondary_linked && sc_rate_limited && secondary_is_sc;

        let primary_outcome = if primary_linked || primary_skipped_sc {
            None
        } else {
            Some(evaluate_provider(pool, library_id, series_name, &primary_name, false).await)
        };

        let secondary_outcome = match secondary_name.as_deref() {
            Some(provider) if !secondary_linked && !(sc_rate_limited && secondary_is_sc) => {
                Some(evaluate_provider(pool, library_id, series_name, provider, true).await)
            }
            _ => None,
        };

        // Detect SensCritique 429 on either provider and trip the job circuit
        // breaker for the remaining series.
        for outcome in primary_outcome.iter().chain(secondary_outcome.iter()) {
            if outcome.status == "error"
                && outcome
                    .message
                    .as_deref()
                    .is_some_and(|e| e.contains(RATE_LIMITED_ERROR))
            {
                sc_rate_limited = true;
                warn!(
                    "[METADATA_BATCH] job={job_id} SensCritique rate-limited on '{series_name}', skipping remaining SC lookups"
                );
            }
        }

        // A confident primary wins; otherwise a confident secondary (fallback)
        // becomes the reported match. A series that already has its primary
        // link must not be downgraded just because the secondary found
        // nothing, and a rate-limited lookup only fails a series with no link.
        let selected = select_series_outcome(
            primary_linked,
            secondary_linked,
            primary_skipped_sc || secondary_skipped_sc,
            primary_outcome.as_ref(),
            secondary_outcome.as_ref(),
        );

        let (main, main_is_secondary) = match selected {
            SeriesOutcome::Reported {
                main,
                from_secondary,
            } => (main, from_secondary),
            SeriesOutcome::AlreadyLinked => {
                let mut detail = serde_json::json!({
                    "primary_linked": primary_linked,
                    "secondary_linked": secondary_linked,
                });
                if let Some(secondary) = &secondary_outcome {
                    detail["secondary"] = secondary.detail();
                }
                if secondary_skipped_sc {
                    detail["secondary_skipped"] = serde_json::json!("rate_limited");
                }
                processed += 1;
                update_progress(pool, job_id, processed, total, series_name).await;
                insert_event(
                    pool,
                    job_id,
                    "metadata_already_linked",
                    "info",
                    Some("series"),
                    Some(series_name),
                    None,
                    Some(detail),
                )
                .await;
                continue;
            }
            SeriesOutcome::RateLimited => {
                processed += 1;
                update_progress(pool, job_id, processed, total, series_name).await;
                warn!(
                    "[METADATA_BATCH] job={job_id} skipping '{series_name}' (SensCritique rate-limited)"
                );
                insert_event(
                    pool,
                    job_id,
                    "error",
                    "error",
                    Some("series"),
                    Some(series_name),
                    Some("Skipped: SensCritique rate-limited earlier in this job"),
                    None,
                )
                .await;
                continue;
            }
        };

        // The per-provider outcome doubles as the event payload; the outcome
        // that was not selected is attached as a nested object so both
        // providers stay visible (e.g. a primary error under a secondary match).
        let mut detail = main.detail();
        if main_is_secondary {
            if let Some(primary) = &primary_outcome {
                detail["primary"] = primary.detail();
            }
        } else if let Some(secondary) = &secondary_outcome {
            detail["secondary"] = secondary.detail();
        }

        let event_type = match main.status {
            "auto_matched" => "metadata_matched",
            "no_results" => "metadata_no_results",
            "low_confidence" => "metadata_low_confidence",
            "too_many_results" => "metadata_too_many",
            _ => "error",
        };
        let level = match event_type {
            "metadata_low_confidence" | "metadata_too_many" => "warning",
            "error" => "error",
            _ => "info",
        };
        insert_event(
            pool,
            job_id,
            event_type,
            level,
            Some("series"),
            Some(series_name),
            main.message.as_deref(),
            Some(detail),
        )
        .await;

        // In rematch mode, drop links from providers that are neither the
        // primary nor the configured secondary. Keeping the secondary is
        // required now that a single job can match both (#150).
        let rematched = primary_outcome
            .as_ref()
            .is_some_and(|o| o.is_auto_matched())
            || secondary_outcome
                .as_ref()
                .is_some_and(|o| o.is_auto_matched());
        if force_rematch && rematched {
            let mut keep_providers = vec![primary_name.clone()];
            if let Some(secondary) = &secondary_name {
                keep_providers.push(secondary.clone());
            }
            if let Err(e) =
                cleanup_rematch_links(pool, library_id, series_name, &keep_providers).await
            {
                warn!(
                    "[METADATA_BATCH] job={job_id} rematch cleanup failed for '{series_name}': {e}"
                );
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

    complete_job(pool, job_id, stats)
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

    let library_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

    let new_matches: Vec<String> = sqlx::query(
        "SELECT entity_name FROM index_job_events WHERE job_id = $1 AND event_type = 'metadata_matched' ORDER BY created_at LIMIT 10",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter_map(|r| r.try_get::<Option<String>, _>("entity_name").ok().flatten())
    .collect();

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
            new_matches,
        },
    );

    Ok(())
}

use super::batch_sync::{
    cleanup_rematch_links, evaluate_provider, has_approved_link, load_approved_links,
    select_series_outcome, SeriesOutcome,
};

// Helpers moved to crate::job_helpers and super::config

#[cfg(test)]
#[path = "tests/batch.rs"]
mod tests;
