use axum::{
    extract::{Path as AxumPath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use utoipa::ToSchema;
use tracing::{info, warn};

use crate::{error::ApiError, metadata_providers, state::AppState};
use crate::job_helpers::{is_job_cancelled, update_progress};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct MetadataRefreshRequest {
    pub library_id: Option<String>,
}

/// A single field change: old -> new
#[derive(Serialize, Clone)]
struct FieldDiff {
    field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new: Option<serde_json::Value>,
}

/// Per-book changes
#[derive(Serialize, Clone)]
struct BookDiff {
    book_id: String,
    title: String,
    volume: Option<i32>,
    changes: Vec<FieldDiff>,
}

/// Per-series change report
#[derive(Serialize, Clone)]
pub(crate) struct SeriesRefreshResult {
    series_name: String,
    provider: String,
    status: String, // "updated", "unchanged", "error"
    series_changes: Vec<FieldDiff>,
    book_changes: Vec<BookDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
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

/// Refresh a single approved metadata link: re-fetch from provider, compare, sync, return diff
pub(crate) async fn refresh_link(
    pool: &PgPool,
    link_id: Uuid,
    library_id: Uuid,
    series_name: &str,
    provider_name: &str,
    external_id: &str,
) -> Result<SeriesRefreshResult, String> {
    let provider = metadata_providers::get_provider(provider_name)
        .ok_or_else(|| format!("Unknown provider: {provider_name}"))?;

    let config = super::config::load_provider_config(pool, provider_name).await;

    let mut series_changes: Vec<FieldDiff> = Vec::new();
    let mut book_changes: Vec<BookDiff> = Vec::new();

    // -- Series-level refresh --
    let candidates = provider
        .search_series(series_name, &config)
        .await
        .map_err(|e| format!("provider search error: {e}"))?;

    let candidate = candidates
        .iter()
        .find(|c| c.external_id == external_id)
        .or_else(|| candidates.first());

    if let Some(candidate) = candidate {
        // Update link metadata_json
        sqlx::query(
            r#"
            UPDATE external_metadata_links
            SET metadata_json = $2,
                total_volumes_external = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(link_id)
        .bind(&candidate.metadata_json)
        .bind(candidate.total_volumes)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        // Diff + sync series metadata
        series_changes = sync_series_with_diff(pool, library_id, series_name, candidate).await?;
    }

    // -- Book-level refresh --
    let books = provider
        .get_series_books(external_id, &config)
        .await
        .map_err(|e| format!("provider books error: {e}"))?;

    // Delete existing external_book_metadata for this link
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

        // Insert external_book_metadata
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

        // Diff + push metadata to matched local book
        if let Some(book_id) = local_book_id {
            let diffs = sync_book_with_diff(pool, book_id, book).await?;
            if !diffs.is_empty() {
                let local_title = local_books_with_pos
                    .iter()
                    .find(|(id, _, _)| *id == book_id)
                    .map(|(_, _, t)| t.clone())
                    .unwrap_or_default();
                book_changes.push(BookDiff {
                    book_id: book_id.to_string(),
                    title: local_title,
                    volume: book.volume_number,
                    changes: diffs,
                });
            }
        }
    }

    // Update synced_at on the link
    sqlx::query("UPDATE external_metadata_links SET synced_at = NOW(), updated_at = NOW() WHERE id = $1")
        .bind(link_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Re-match any external books that couldn't be matched during insert
    // (e.g., metadata was fetched before books were imported)
    let _ = rematch_unlinked_books(pool, library_id).await;

    let has_changes = !series_changes.is_empty() || !book_changes.is_empty();

    Ok(SeriesRefreshResult {
        series_name: series_name.to_string(),
        provider: provider_name.to_string(),
        status: if has_changes { "updated".to_string() } else { "unchanged".to_string() },
        series_changes,
        book_changes,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// Diff helpers
// ---------------------------------------------------------------------------

/// Compare old/new for a nullable string field. Returns Some(FieldDiff) only if value actually changed.
fn diff_opt_str(field: &str, old: Option<&str>, new: Option<&str>) -> Option<FieldDiff> {
    let new_val = new.filter(|s| !s.is_empty());
    // Only report a change if there is a new non-empty value AND it differs from old
    match (old, new_val) {
        (Some(o), Some(n)) if o != n => Some(FieldDiff {
            field: field.to_string(),
            old: Some(serde_json::Value::String(o.to_string())),
            new: Some(serde_json::Value::String(n.to_string())),
        }),
        (None, Some(n)) => Some(FieldDiff {
            field: field.to_string(),
            old: None,
            new: Some(serde_json::Value::String(n.to_string())),
        }),
        _ => None,
    }
}

fn diff_opt_i32(field: &str, old: Option<i32>, new: Option<i32>) -> Option<FieldDiff> {
    match (old, new) {
        (Some(o), Some(n)) if o != n => Some(FieldDiff {
            field: field.to_string(),
            old: Some(serde_json::json!(o)),
            new: Some(serde_json::json!(n)),
        }),
        (None, Some(n)) => Some(FieldDiff {
            field: field.to_string(),
            old: None,
            new: Some(serde_json::json!(n)),
        }),
        _ => None,
    }
}

fn diff_str_vec(field: &str, old: &[String], new: &[String]) -> Option<FieldDiff> {
    if new.is_empty() {
        return None;
    }
    if old != new {
        Some(FieldDiff {
            field: field.to_string(),
            old: Some(serde_json::json!(old)),
            new: Some(serde_json::json!(new)),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Series sync with diff tracking
// ---------------------------------------------------------------------------

async fn sync_series_with_diff(
    pool: &PgPool,
    library_id: Uuid,
    series_name: &str,
    candidate: &metadata_providers::SeriesCandidate,
) -> Result<Vec<FieldDiff>, String> {
    let new_description = candidate.metadata_json
        .get("description")
        .and_then(|d| d.as_str())
        .or(candidate.description.as_deref());
    let new_authors = &candidate.authors;
    let new_publishers = &candidate.publishers;
    let new_start_year = candidate.start_year;
    let new_total_volumes = candidate.total_volumes;
    let new_status = if let Some(raw) = candidate.metadata_json.get("status").and_then(|s| s.as_str()) {
        Some(super::handlers::normalize_series_status(pool, raw).await)
    } else {
        None
    };
    let new_status = new_status.as_deref();

    // Fetch existing series metadata for diffing
    let existing = sqlx::query(
        r#"SELECT description, publishers, start_year, total_volumes, status, authors, locked_fields
           FROM series WHERE library_id = $1 AND name = $2"#,
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let locked = existing
        .as_ref()
        .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
        .unwrap_or(serde_json::json!({}));
    let is_locked = |field: &str| -> bool {
        locked.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
    };

    // Build diffs (only for unlocked fields that actually change)
    let mut diffs: Vec<FieldDiff> = Vec::new();

    if !is_locked("description") {
        let old_desc: Option<String> = existing.as_ref().and_then(|r| r.get("description"));
        if let Some(d) = diff_opt_str("description", old_desc.as_deref(), new_description) {
            diffs.push(d);
        }
    }
    if !is_locked("authors") {
        let old_authors: Vec<String> = existing.as_ref().map(|r| r.get("authors")).unwrap_or_default();
        if let Some(d) = diff_str_vec("authors", &old_authors, new_authors) {
            diffs.push(d);
        }
    }
    if !is_locked("publishers") {
        let old_publishers: Vec<String> = existing.as_ref().map(|r| r.get("publishers")).unwrap_or_default();
        if let Some(d) = diff_str_vec("publishers", &old_publishers, new_publishers) {
            diffs.push(d);
        }
    }
    if !is_locked("start_year") {
        let old_year: Option<i32> = existing.as_ref().and_then(|r| r.get("start_year"));
        if let Some(d) = diff_opt_i32("start_year", old_year, new_start_year) {
            diffs.push(d);
        }
    }
    if !is_locked("total_volumes") {
        let old_vols: Option<i32> = existing.as_ref().and_then(|r| r.get("total_volumes"));
        if let Some(d) = diff_opt_i32("total_volumes", old_vols, new_total_volumes) {
            diffs.push(d);
        }
    }
    if !is_locked("status") {
        let old_status: Option<String> = existing.as_ref().and_then(|r| r.get("status"));
        if let Some(d) = diff_opt_str("status", old_status.as_deref(), new_status) {
            diffs.push(d);
        }
    }

    // Now do the actual upsert
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
    .bind(new_description)
    .bind(new_publishers)
    .bind(new_start_year)
    .bind(new_total_volumes)
    .bind(new_status)
    .bind(new_authors)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(diffs)
}

// ---------------------------------------------------------------------------
// Book sync with diff tracking
// ---------------------------------------------------------------------------

async fn sync_book_with_diff(
    pool: &PgPool,
    book_id: Uuid,
    ext_book: &metadata_providers::BookCandidate,
) -> Result<Vec<FieldDiff>, String> {
    // Fetch current book state
    let current = sqlx::query(
        "SELECT summary, isbn, publish_date, language, authors, locked_fields FROM books WHERE id = $1",
    )
    .bind(book_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let locked = current.get::<serde_json::Value, _>("locked_fields");
    let is_locked = |field: &str| -> bool {
        locked.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
    };

    // Build diffs
    let mut diffs: Vec<FieldDiff> = Vec::new();

    if !is_locked("summary") {
        let old: Option<String> = current.get("summary");
        if let Some(d) = diff_opt_str("summary", old.as_deref(), ext_book.summary.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("isbn") {
        let old: Option<String> = current.get("isbn");
        if let Some(d) = diff_opt_str("isbn", old.as_deref(), ext_book.isbn.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("publish_date") {
        let old: Option<String> = current.get("publish_date");
        if let Some(d) = diff_opt_str("publish_date", old.as_deref(), ext_book.publish_date.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("language") {
        let old: Option<String> = current.get("language");
        if let Some(d) = diff_opt_str("language", old.as_deref(), ext_book.language.as_deref()) {
            diffs.push(d);
        }
    }
    if !is_locked("authors") {
        let old: Vec<String> = current.get("authors");
        if let Some(d) = diff_str_vec("authors", &old, &ext_book.authors) {
            diffs.push(d);
        }
    }

    // Do the actual update
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
    .bind(&ext_book.summary)
    .bind(&ext_book.isbn)
    .bind(&ext_book.publish_date)
    .bind(&ext_book.language)
    .bind(&ext_book.authors)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(diffs)
}

/// Re-match external_book_metadata rows that have book_id IS NULL
/// by joining on volume number with local books in the same series.
/// Called after scans/imports to fix metadata that was fetched before books existed.
pub async fn rematch_unlinked_books(pool: &PgPool, library_id: Uuid) -> Result<i64, String> {
    let result = sqlx::query(
        r#"
        UPDATE external_book_metadata ebm
        SET book_id = matched.book_id
        FROM (
            SELECT DISTINCT ON (ebm2.id)
                ebm2.id AS ebm_id,
                b.id AS book_id
            FROM external_book_metadata ebm2
            JOIN external_metadata_links eml ON eml.id = ebm2.link_id
            JOIN books b ON b.library_id = eml.library_id
                AND b.series_id = eml.series_id
                AND b.volume = ebm2.volume_number
            WHERE eml.library_id = $1
              AND ebm2.book_id IS NULL
              AND ebm2.volume_number IS NOT NULL
              AND eml.status = 'approved'
        ) matched
        WHERE ebm.id = matched.ebm_id
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let count = result.rows_affected() as i64;
    if count > 0 {
        info!("[METADATA] Re-matched {count} unlinked external books for library {library_id}");
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_dto_serializes() {
        let dto = MetadataRefreshReportDto {
            job_id: Uuid::nil(),
            status: "success".to_string(),
            total_links: 10,
            refreshed: 3,
            unchanged: 6,
            errors: 1,
            changes: serde_json::json!([]),
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["status"], "success");
        assert_eq!(json["total_links"], 10);
        assert_eq!(json["refreshed"], 3);
        assert_eq!(json["unchanged"], 6);
        assert_eq!(json["errors"], 1);
        assert_eq!(json["changes"], serde_json::json!([]));
        assert_eq!(json["job_id"], Uuid::nil().to_string());
    }

    #[test]
    fn series_refresh_result_serializes_updated() {
        let result = SeriesRefreshResult {
            series_name: "Blacksad".to_string(),
            provider: "google_books".to_string(),
            status: "updated".to_string(),
            series_changes: vec![FieldDiff {
                field: "title".to_string(),
                old: Some(serde_json::json!("Blacksad old")),
                new: Some(serde_json::json!("Blacksad")),
            }],
            book_changes: vec![],
            error: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["series_name"], "Blacksad");
        assert_eq!(json["status"], "updated");
        assert_eq!(json["series_changes"][0]["field"], "title");
        // error field should be absent (skip_serializing_if = None)
        assert!(json.get("error").is_none());
    }

    #[test]
    fn series_refresh_result_serializes_error() {
        let result = SeriesRefreshResult {
            series_name: "Test".to_string(),
            provider: "anilist".to_string(),
            status: "error".to_string(),
            series_changes: vec![],
            book_changes: vec![],
            error: Some("provider timeout".to_string()),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["error"], "provider timeout");
    }

    #[test]
    fn field_diff_skips_none_values() {
        let diff = FieldDiff {
            field: "summary".to_string(),
            old: None,
            new: Some(serde_json::json!("A new summary")),
        };
        let json = serde_json::to_value(&diff).unwrap();
        assert!(json.get("old").is_none());
        assert_eq!(json["new"], "A new summary");
    }

    #[test]
    fn book_diff_serializes() {
        let diff = BookDiff {
            book_id: "abc-123".to_string(),
            title: "Volume 1".to_string(),
            volume: Some(1),
            changes: vec![FieldDiff {
                field: "page_count".to_string(),
                old: Some(serde_json::json!(100)),
                new: Some(serde_json::json!(120)),
            }],
        };
        let json = serde_json::to_value(&diff).unwrap();
        assert_eq!(json["book_id"], "abc-123");
        assert_eq!(json["volume"], 1);
        assert_eq!(json["changes"][0]["old"], 100);
        assert_eq!(json["changes"][0]["new"], 120);
    }

    #[test]
    fn request_dto_deserializes_without_library_id() {
        let json = serde_json::json!({});
        let req: MetadataRefreshRequest = serde_json::from_value(json).unwrap();
        assert!(req.library_id.is_none());
    }

    #[test]
    fn request_dto_deserializes_with_library_id() {
        let json = serde_json::json!({"library_id": "abc-123"});
        let req: MetadataRefreshRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.library_id.unwrap(), "abc-123");
    }
}
