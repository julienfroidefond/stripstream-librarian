use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use utoipa::ToSchema;
use tracing::{info, warn};

use crate::{error::ApiError, state::AppState};
use crate::job_helpers::{is_job_cancelled, update_progress};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct MetadataRefreshRequest {
    pub library_id: Option<String>,
}

use super::refresh_sync::{FieldDiff, BookDiff};
pub(crate) use super::refresh_sync::refresh_link;

/// Per-series change report
#[derive(Serialize, Clone)]
pub(crate) struct SeriesRefreshResult {
    pub(crate) series_name: String,
    pub(crate) provider: String,
    pub(crate) status: String, // "updated", "unchanged", "error"
    pub(crate) series_changes: Vec<FieldDiff>,
    pub(crate) book_changes: Vec<BookDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
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
        (status = 200, description = "Job created"),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_refresh(
    State(state): State<AppState>,
    Json(body): Json<MetadataRefreshRequest>,
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
            let link_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM external_metadata_links eml
                LEFT JOIN series sm
                    ON sm.library_id = eml.library_id AND sm.id = eml.series_id
                WHERE eml.library_id = $1
                  AND eml.status = 'approved'
                  AND COALESCE(sm.status, 'ongoing') NOT IN ('ended', 'cancelled')
                "#,
            )
            .bind(library_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
            if link_count == 0 { continue; }
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'metadata_refresh' AND status IN ('pending', 'running') LIMIT 1",
            )
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?;
            if existing.is_some() { continue; }
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_refresh', 'running', NOW())",
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
                if let Err(e) = process_metadata_refresh(&pool, job_id, library_id).await {
                    warn!("[METADATA_REFRESH] job {job_id} failed: {e}");
                    let _ = sqlx::query(
                        "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
                    )
                    .bind(job_id)
                    .bind(e.to_string())
                    .execute(&pool)
                    .await;
                    notifications::notify(
                        pool.clone(),
                        notifications::NotificationEvent::MetadataRefreshFailed {
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

    // Check no existing running metadata_refresh job for this library
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'metadata_refresh' AND status IN ('pending', 'running') LIMIT 1",
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

    // Check there are approved links to refresh (only ongoing series)
    let link_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM external_metadata_links eml
        LEFT JOIN series sm
            ON sm.library_id = eml.library_id AND sm.id = eml.series_id
        WHERE eml.library_id = $1
          AND eml.status = 'approved'
          AND COALESCE(sm.status, 'ongoing') NOT IN ('ended', 'cancelled')
        "#,
    )
    .bind(library_id)
    .fetch_one(&state.pool)
    .await?;

    if link_count == 0 {
        return Err(ApiError::bad_request("No approved metadata links to refresh for this library"));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_refresh', 'running', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
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
        if let Err(e) = process_metadata_refresh(&pool, job_id, library_id).await {
            warn!("[METADATA_REFRESH] job {job_id} failed: {e}");
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .bind(e.to_string())
            .execute(&pool)
            .await;
            notifications::notify(
                pool.clone(),
                notifications::NotificationEvent::MetadataRefreshFailed {
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
// POST /metadata/refresh-all — Trigger a metadata refresh for ALL series (including ended/cancelled)
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/metadata/refresh-all",
    tag = "metadata",
    request_body = MetadataRefreshRequest,
    responses(
        (status = 200, description = "Job created"),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_refresh_all(
    State(state): State<AppState>,
    Json(body): Json<MetadataRefreshRequest>,
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
            let link_count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM external_metadata_links eml
                WHERE eml.library_id = $1
                  AND eml.status = 'approved'
                "#,
            )
            .bind(library_id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
            if link_count == 0 { continue; }
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'metadata_refresh_all' AND status IN ('pending', 'running') LIMIT 1",
            )
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?;
            if existing.is_some() { continue; }
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_refresh_all', 'running', NOW())",
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
                if let Err(e) = process_metadata_refresh_all(&pool, job_id, library_id).await {
                    warn!("[METADATA_REFRESH_ALL] job {job_id} failed: {e}");
                    let _ = sqlx::query(
                        "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
                    )
                    .bind(job_id)
                    .bind(e.to_string())
                    .execute(&pool)
                    .await;
                    notifications::notify(
                        pool.clone(),
                        notifications::NotificationEvent::MetadataRefreshFailed {
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

    // Check no existing running metadata_refresh_all job for this library
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'metadata_refresh_all' AND status IN ('pending', 'running') LIMIT 1",
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

    // Check there are approved links to refresh (ALL series, no status filter)
    let link_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM external_metadata_links eml
        WHERE eml.library_id = $1
          AND eml.status = 'approved'
        "#,
    )
    .bind(library_id)
    .fetch_one(&state.pool)
    .await?;

    if link_count == 0 {
        return Err(ApiError::bad_request("No approved metadata links to refresh for this library"));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'metadata_refresh_all', 'running', NOW())",
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
        if let Err(e) = process_metadata_refresh_all(&pool, job_id, library_id).await {
            warn!("[METADATA_REFRESH_ALL] job {job_id} failed: {e}");
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .bind(e.to_string())
            .execute(&pool)
            .await;
            notifications::notify(
                pool.clone(),
                notifications::NotificationEvent::MetadataRefreshFailed {
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
        return Err(ApiError::bad_request("only approved links can be refreshed"));
    }

    let library_id: Uuid = row.get("library_id");
    let series_name: String = row.get("series_name");
    let provider: String = row.get("provider");
    let external_id: String = row.get("external_id");

    match refresh_link(&state.pool, link_id, library_id, &series_name, &provider, &external_id).await {
        Ok(result) => {
            Ok(Json(serde_json::json!({
                "ok": true,
                "status": result.status,
            })))
        }
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
    sqlx::query("UPDATE index_jobs SET status = 'running', started_at = NOW() WHERE id = $1")
        .bind(job_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Get approved links for this library
    let query = if include_all {
        // Refresh ALL series regardless of status
        r#"
        SELECT eml.id, sm.name AS series_name, eml.provider, eml.external_id
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
        SELECT eml.id, sm.name AS series_name, eml.provider, eml.external_id
        FROM external_metadata_links eml
        JOIN series sm
            ON sm.id = eml.series_id
        WHERE eml.library_id = $1
          AND eml.status = 'approved'
          AND COALESCE(sm.status, 'ongoing') NOT IN ('ended', 'cancelled')
        ORDER BY sm.name
        "#
    };
    let links: Vec<(Uuid, String, String, String)> = sqlx::query_as(query)
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let total = links.len() as i32;
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut processed = 0i32;
    let mut refreshed = 0i32;
    let mut unchanged = 0i32;
    let mut errors = 0i32;
    let mut all_results: Vec<SeriesRefreshResult> = Vec::new();

    let mut last_provider: Option<String> = None;
    for (link_id, series_name, provider_name, external_id) in &links {
        // Throttle SensCritique requests to avoid 429
        if provider_name == "senscritique" && last_provider.as_deref() == Some("senscritique") {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        last_provider = Some(provider_name.clone());

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

        match refresh_link(pool, *link_id, library_id, series_name, provider_name, external_id).await {
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
                errors += 1;
                warn!("[METADATA_REFRESH] job={job_id} error on series='{series_name}': {e}");
                all_results.push(SeriesRefreshResult {
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

    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, stats_json = $2 WHERE id = $1",
    )
    .bind(job_id)
    .bind(stats)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    info!("[METADATA_REFRESH] job={job_id} completed: {refreshed} updated, {unchanged} unchanged, {errors} errors");

    let library_name: Option<String> = sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    // Compute detailed stats for notification
    let mut series_fields_count = 0usize;
    let mut books_fields_count = 0usize;
    let mut detail_lines: Vec<String> = Vec::new();
    for result in &all_results {
        if result.status == "updated" {
            let series_field_names: Vec<&str> = result.series_changes.iter().map(|c| c.field.as_str()).collect();
            let book_field_count = result.book_changes.iter().map(|b| b.changes.len()).sum::<usize>();
            series_fields_count += series_field_names.len();
            books_fields_count += book_field_count;
            if !series_field_names.is_empty() || book_field_count > 0 {
                let mut parts: Vec<String> = Vec::new();
                if !series_field_names.is_empty() {
                    parts.push(series_field_names.join(", "));
                }
                if book_field_count > 0 {
                    parts.push(format!("{book_field_count} book fields"));
                }
                detail_lines.push(format!("{}: {}", result.series_name, parts.join(", ")));
            }
        }
    }

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
        },
    );

    Ok(())
}

// refresh_link, diff helpers, sync_series_with_diff, sync_book_with_diff, rematch_unlinked_books
// are now in super::refresh_sync

#[cfg(test)]
#[path = "tests/refresh.rs"]
mod tests;
