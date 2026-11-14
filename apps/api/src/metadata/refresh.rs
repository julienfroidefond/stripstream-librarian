use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::job_helpers::{is_job_cancelled, set_total_files, update_progress};
use crate::jobs::launcher::{self, JobLaunchResponse, JobSpec};
use crate::jobs::lifecycle::{cancel_job, classify_job, finish_job, start_job, JobStatus};
use crate::metadata_providers::senscritique::RATE_LIMITED_ERROR;
use crate::{error::ApiError, state::AppState};
use notifications::NotificationEvent;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct MetadataRefreshRequest {
    pub library_id: Option<String>,
}

pub(crate) use super::refresh_sync::refresh_link;
use super::refresh_sync::{BookDiff, FieldDiff};

/// Per-series change report
#[derive(Serialize, Clone)]
pub(crate) struct SeriesRefreshResult {
    /// Local series id, so the job report can link to the series page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) series_id: Option<Uuid>,
    pub(crate) series_name: String,
    pub(crate) provider: String,
    pub(crate) status: String, // "updated", "unchanged", "error"
    pub(crate) series_changes: Vec<FieldDiff>,
    pub(crate) book_changes: Vec<BookDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

const NOTIFICATION_VALUE_MAX_LEN: usize = 120;

fn format_notification_value(value: Option<&serde_json::Value>) -> String {
    let value = match value {
        None | Some(serde_json::Value::Null) => "∅".to_string(),
        Some(serde_json::Value::String(value)) => value.clone(),
        Some(value) => value.to_string(),
    };
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");

    if value.chars().count() > NOTIFICATION_VALUE_MAX_LEN {
        format!(
            "{}…",
            value
                .chars()
                .take(NOTIFICATION_VALUE_MAX_LEN)
                .collect::<String>()
        )
    } else {
        value
    }
}

fn format_notification_change(change: &FieldDiff) -> String {
    format!(
        "{}: {} → {}",
        change.field,
        format_notification_value(change.old.as_ref()),
        format_notification_value(change.new.as_ref()),
    )
}

/// Response DTO for the report endpoint
#[derive(Serialize, ToSchema)]
pub struct MetadataRefreshReportDto {
    #[schema(value_type = String)]
    pub job_id: Uuid,
    pub status: String,
    pub total_links: i64,
    pub refreshed: i64,
    pub unchanged: i64,
    pub errors: i64,
    pub changes: serde_json::Value,
}

// ---------------------------------------------------------------------------
// POST /metadata/refresh — Trigger a metadata refresh job
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/refresh",
    tag = "metadata",
    request_body = MetadataRefreshRequest,
    responses(
        (status = 200, body = JobLaunchResponse),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_refresh(
    State(state): State<AppState>,
    Json(body): Json<MetadataRefreshRequest>,
) -> Result<Json<JobLaunchResponse>, ApiError> {
    launch_refresh(&state.pool, body, RefreshScope::Ongoing).await
}

// ---------------------------------------------------------------------------
// POST /metadata/refresh-all — Trigger a metadata refresh for ALL series (including ended/cancelled)
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/refresh-all",
    tag = "metadata",
    request_body = MetadataRefreshRequest,
    responses(
        (status = 200, body = JobLaunchResponse),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_refresh_all(
    State(state): State<AppState>,
    Json(body): Json<MetadataRefreshRequest>,
) -> Result<Json<JobLaunchResponse>, ApiError> {
    launch_refresh(&state.pool, body, RefreshScope::All).await
}

/// Which approved links a refresh job walks through.
#[derive(Clone, Copy)]
enum RefreshScope {
    /// `metadata_refresh`: only series that are not ended or cancelled.
    Ongoing,
    /// `metadata_refresh_all`: every series.
    All,
}

impl RefreshScope {
    fn spec(self) -> &'static JobSpec {
        match self {
            RefreshScope::Ongoing => &METADATA_REFRESH,
            RefreshScope::All => &METADATA_REFRESH_ALL,
        }
    }
}

pub(crate) const METADATA_REFRESH: JobSpec = JobSpec {
    job_type: "metadata_refresh",
    conflicts: &[],
    failure: Some(refresh_failed),
    partial_stats: None,
};

pub(crate) const METADATA_REFRESH_ALL: JobSpec = JobSpec {
    job_type: "metadata_refresh_all",
    conflicts: &[],
    failure: Some(refresh_failed),
    partial_stats: None,
};

fn refresh_failed(library_name: Option<String>, error: String) -> NotificationEvent {
    NotificationEvent::MetadataRefreshFailed {
        library_name,
        error,
    }
}

/// Number of approved links a refresh of `scope` would walk for a library.
async fn refreshable_link_count(
    pool: &PgPool,
    library_id: Uuid,
    scope: RefreshScope,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM external_metadata_links eml
        LEFT JOIN series sm
            ON sm.library_id = eml.library_id AND sm.id = eml.series_id
        WHERE eml.library_id = $1
          AND eml.status = 'approved'
          AND ($2 OR COALESCE(sm.status, 'ongoing') NOT IN ('ended', 'cancelled'))
        "#,
    )
    .bind(library_id)
    .bind(matches!(scope, RefreshScope::All))
    .fetch_one(pool)
    .await
}

async fn run_refresh(
    pool: PgPool,
    job_id: Uuid,
    library_id: Uuid,
    scope: RefreshScope,
) -> Result<(), String> {
    match scope {
        RefreshScope::Ongoing => process_metadata_refresh(&pool, job_id, library_id).await,
        RefreshScope::All => process_metadata_refresh_all(&pool, job_id, library_id).await,
    }
}

async fn launch_refresh(
    pool: &PgPool,
    body: MetadataRefreshRequest,
    scope: RefreshScope,
) -> Result<Json<JobLaunchResponse>, ApiError> {
    let spec = scope.spec();

    // All libraries case: only libraries with something to refresh
    let Some(library_id) = body.library_id else {
        let candidates: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM libraries WHERE metadata_provider IS DISTINCT FROM 'none' ORDER BY name",
        )
        .fetch_all(pool)
        .await?;
        let mut library_ids = Vec::with_capacity(candidates.len());
        for library_id in candidates {
            if refreshable_link_count(pool, library_id, scope).await? > 0 {
                library_ids.push(library_id);
            }
        }
        let fan_out =
            launcher::launch_each(pool, library_ids, spec, move |pool, job_id, library_id| {
                run_refresh(pool, job_id, library_id, scope)
            })
            .await?;
        return Ok(Json(fan_out.into()));
    };

    let library_id: Uuid = library_id
        .parse()
        .map_err(|_| ApiError::bad_request("invalid library_id"))?;

    // Verify library exists
    sqlx::query("SELECT 1 FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("library not found"))?;

    if refreshable_link_count(pool, library_id, scope).await? == 0 {
        return Err(ApiError::bad_request(
            "No approved metadata links to refresh for this library",
        ));
    }

    let launched = launcher::launch(pool, Some(library_id), spec, move |pool, job_id| {
        run_refresh(pool, job_id, library_id, scope)
    })
    .await?;
    Ok(Json(launched.into()))
}

// ---------------------------------------------------------------------------
// GET /metadata/refresh/:id/report — Refresh report from stats_json
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/metadata/refresh/{id}/report",
    tag = "metadata",
    params(("id" = String, Path, description = "Job UUID")),
    responses(
        (status = 200, body = MetadataRefreshReportDto),
        (status = 404, description = "Job not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_refresh_report(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<Uuid>,
) -> Result<Json<MetadataRefreshReportDto>, ApiError> {
    let row = sqlx::query(
        "SELECT status, stats_json, total_files FROM index_jobs WHERE id = $1 AND type IN ('metadata_refresh', 'metadata_refresh_all')",
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("job not found"))?;

    let job_status: String = row.get("status");
    let stats: Option<serde_json::Value> = row.get("stats_json");
    let total_files: Option<i32> = row.get("total_files");

    let (refreshed, unchanged, errors, changes) = if let Some(ref s) = stats {
        (
            s.get("refreshed").and_then(|v| v.as_i64()).unwrap_or(0),
            s.get("unchanged").and_then(|v| v.as_i64()).unwrap_or(0),
            s.get("errors").and_then(|v| v.as_i64()).unwrap_or(0),
            s.get("changes").cloned().unwrap_or(serde_json::json!([])),
        )
    } else {
        (0, 0, 0, serde_json::json!([]))
    };

    Ok(Json(MetadataRefreshReportDto {
        job_id,
        status: job_status,
        total_links: total_files.unwrap_or(0) as i64,
        refreshed,
        unchanged,
        errors,
        changes,
    }))
}

// ---------------------------------------------------------------------------
// POST /metadata/refresh-link/:id — Refresh a single metadata link
// ---------------------------------------------------------------------------

/// Refresh a single approved metadata link by its ID.
pub async fn refresh_single_link(
    State(state): State<AppState>,
    AxumPath(link_id): AxumPath<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query(
        "SELECT eml.library_id, s.name AS series_name, eml.provider, eml.external_id, eml.status \
         FROM external_metadata_links eml \
         JOIN series s ON s.id = eml.series_id \
         WHERE eml.id = $1",
    )
    .bind(link_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("metadata link not found"))?;

    let status: String = row.get("status");
    if status != "approved" {
        return Err(ApiError::bad_request(
            "only approved links can be refreshed",
        ));
    }

    let library_id: Uuid = row.get("library_id");
    let series_name: String = row.get("series_name");
    let provider: String = row.get("provider");
    let external_id: String = row.get("external_id");

    match refresh_link(
        &state.pool,
        link_id,
        library_id,
        &series_name,
        &provider,
        &external_id,
    )
    .await
    {
        Ok(result) => Ok(Json(serde_json::json!({
            "ok": true,
            "status": result.status,
        }))),
        Err(e) => Err(ApiError::internal(format!("refresh failed: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

pub(crate) async fn process_metadata_refresh(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(), String> {
    process_metadata_refresh_inner(pool, job_id, library_id, false).await
}

pub(crate) async fn process_metadata_refresh_all(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(), String> {
    process_metadata_refresh_inner(pool, job_id, library_id, true).await
}

async fn process_metadata_refresh_inner(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
    include_all: bool,
) -> Result<(), String> {
    // Set job to running
    start_job(pool, job_id).await.map_err(|e| e.to_string())?;

    // Get approved links for this library
    let query = if include_all {
        // Refresh ALL series regardless of status
        r#"
        SELECT eml.id, sm.name AS series_name, eml.provider, eml.external_id, eml.series_id
        FROM external_metadata_links eml
        JOIN series sm
            ON sm.id = eml.series_id
        WHERE eml.library_id = $1
          AND eml.status = 'approved'
        ORDER BY sm.name
        "#
    } else {
        // Only ongoing series (not ended/cancelled)
        r#"
        SELECT eml.id, sm.name AS series_name, eml.provider, eml.external_id, eml.series_id
        FROM external_metadata_links eml
        JOIN series sm
            ON sm.id = eml.series_id
        WHERE eml.library_id = $1
          AND eml.status = 'approved'
          AND COALESCE(sm.status, 'ongoing') NOT IN ('ended', 'cancelled')
        ORDER BY sm.name
        "#
    };
    let links: Vec<(Uuid, String, String, String, Uuid)> = sqlx::query_as(query)
        .bind(library_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let total = links.len() as i32;
    set_total_files(pool, job_id, total)
        .await
        .map_err(|e| e.to_string())?;

    let mut processed = 0i32;
    let mut refreshed = 0i32;
    let mut unchanged = 0i32;
    let mut errors = 0i32;
    let mut all_results: Vec<SeriesRefreshResult> = Vec::new();

    let mut sc_rate_limited = false;
    for (link_id, series_name, provider_name, external_id, series_id) in &links {
        // Check cancellation
        if is_job_cancelled(pool, job_id).await {
            cancel_job(pool, job_id).await.map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Circuit breaker: skip remaining SensCritique links after a 429
        if provider_name == "senscritique" && sc_rate_limited {
            warn!("[METADATA_REFRESH] job={job_id} skipping '{series_name}' (SensCritique rate-limited)");
            errors += 1;
            all_results.push(SeriesRefreshResult {
                series_id: Some(*series_id),
                series_name: series_name.clone(),
                provider: provider_name.clone(),
                status: "error".to_string(),
                series_changes: vec![],
                book_changes: vec![],
                error: Some("Skipped: SensCritique rate-limited earlier in this job".to_string()),
            });
            processed += 1;
            update_progress(pool, job_id, processed, total, series_name).await;
            continue;
        }

        match refresh_link(
            pool,
            *link_id,
            library_id,
            series_name,
            provider_name,
            external_id,
        )
        .await
        {
            Ok(result) => {
                if result.status == "updated" {
                    refreshed += 1;
                    info!("[METADATA_REFRESH] job={job_id} updated series='{series_name}' via {provider_name}");
                } else {
                    unchanged += 1;
                }
                all_results.push(result);
            }
            Err(e) => {
                if provider_name == "senscritique" && e.contains(RATE_LIMITED_ERROR) {
                    sc_rate_limited = true;
                    warn!("[METADATA_REFRESH] job={job_id} SensCritique rate-limited on '{series_name}', skipping remaining SC links");
                } else {
                    warn!("[METADATA_REFRESH] job={job_id} error on series='{series_name}': {e}");
                }
                errors += 1;
                all_results.push(SeriesRefreshResult {
                    series_id: Some(*series_id),
                    series_name: series_name.clone(),
                    provider: provider_name.clone(),
                    status: "error".to_string(),
                    series_changes: vec![],
                    book_changes: vec![],
                    error: Some(e),
                });
            }
        }

        processed += 1;
        update_progress(pool, job_id, processed, total, series_name).await;

        // Rate limit: 1s delay between provider calls
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    // Only keep series that have changes or errors (filter out "unchanged")
    let changes_only: Vec<&SeriesRefreshResult> = all_results
        .iter()
        .filter(|r| r.status != "unchanged")
        .collect();

    // Build stats summary
    let stats = serde_json::json!({
        "total_links": total,
        "refreshed": refreshed,
        "unchanged": unchanged,
        "errors": errors,
        "changes": changes_only,
    });

    let (status, error_summary) = finish_refresh_job(pool, job_id, total, errors, stats).await?;

    info!("[METADATA_REFRESH] job={job_id} completed: {refreshed} updated, {unchanged} unchanged, {errors} errors ({status:?})");

    let library_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    // Compute detailed stats for notification
    let mut series_fields_count = 0usize;
    let mut books_fields_count = 0usize;
    let mut detail_lines: Vec<String> = Vec::new();
    let mut error_lines: Vec<String> = Vec::new();
    for result in &all_results {
        if result.status == "updated" {
            let series_field_count = result.series_changes.len();
            let book_field_count = result
                .book_changes
                .iter()
                .map(|b| b.changes.len())
                .sum::<usize>();
            series_fields_count += series_field_count;
            books_fields_count += book_field_count;
            if series_field_count > 0 || book_field_count > 0 {
                let mut changes: Vec<String> = result
                    .series_changes
                    .iter()
                    .map(format_notification_change)
                    .collect();
                for book in &result.book_changes {
                    let book_name = if book.title.is_empty() {
                        format!("Volume {}", book.volume.unwrap_or_default())
                    } else {
                        book.title.clone()
                    };
                    changes.extend(book.changes.iter().map(|change| {
                        format!("{book_name} — {}", format_notification_change(change))
                    }));
                }
                detail_lines.push(format!(
                    "{}:\n  • {}",
                    result.series_name,
                    changes.join("\n  • ")
                ));
            }
        } else if result.status == "error" {
            let error = result.error.as_deref().unwrap_or("Unknown error");
            error_lines.push(format!("{} — {error}", result.series_name));
        }
    }

    // One notification per run: a fully failed refresh reports the failure,
    // otherwise the completion event carries the error counts and details.
    if status == JobStatus::Failed {
        notifications::notify(
            pool.clone(),
            notifications::NotificationEvent::MetadataRefreshFailed {
                library_name,
                error: error_summary
                    .unwrap_or_else(|| format!("{errors}/{total} series en erreur")),
            },
        );
    } else {
        notifications::notify(
            pool.clone(),
            notifications::NotificationEvent::MetadataRefreshCompleted {
                library_name,
                refreshed,
                unchanged,
                errors,
                series_fields_updated: series_fields_count,
                books_fields_updated: books_fields_count,
                details: detail_lines,
                error_details: error_lines,
            },
        );
    }

    Ok(())
}

// refresh_link, diff helpers, sync_series_with_diff, sync_book_with_diff, rematch_unlinked_books
// are now in super::refresh_sync

/// Classify and close a metadata refresh job. Extracted so the partial/success
/// boundary is covered by a DB test without hitting a provider.
async fn finish_refresh_job(
    pool: &PgPool,
    job_id: Uuid,
    total: i32,
    errors: i32,
    stats: serde_json::Value,
) -> Result<(JobStatus, Option<String>), String> {
    let status = classify_job(total.max(0) as usize, errors.max(0) as usize);
    let error_summary =
        (status != JobStatus::Success).then(|| format!("{errors}/{total} series en erreur"));

    finish_job(pool, job_id, status, stats, error_summary.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    Ok((status, error_summary))
}

#[cfg(test)]
#[path = "tests/refresh.rs"]
mod tests;
