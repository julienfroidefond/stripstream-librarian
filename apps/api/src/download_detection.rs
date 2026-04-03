use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, prowlarr, state::AppState};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize, ToSchema)]
pub struct StartDownloadDetectionRequest {
    pub library_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct DownloadDetectionReportDto {
    #[schema(value_type = String)]
    pub job_id: Uuid,
    pub status: String,
    pub total_series: i64,
    pub found: i64,
    pub not_found: i64,
    pub no_missing: i64,
    pub no_metadata: i64,
    pub errors: i64,
}

#[derive(Serialize, ToSchema)]
pub struct DownloadDetectionResultDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = Option<String>)]
    pub series_id: Option<Uuid>,
    pub series_name: String,
    /// 'found' | 'not_found' | 'no_missing' | 'no_metadata' | 'error'
    pub status: String,
    pub missing_count: i32,
    pub available_releases: Option<Vec<AvailableReleaseDto>>,
    pub error_message: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AvailableReleaseDto {
    pub title: String,
    pub size: i64,
    pub download_url: Option<String>,
    pub indexer: Option<String>,
    pub seeders: Option<i32>,
    pub matched_missing_volumes: Vec<i32>,
    #[serde(default)]
    pub all_volumes: Vec<i32>,
    /// True if a previous download of overlapping volumes failed for this series.
    #[serde(default)]
    pub has_failed: bool,
}

// ---------------------------------------------------------------------------
// POST /download-detection/start
// ---------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/download-detection/start",
    tag = "download_detection",
    request_body = StartDownloadDetectionRequest,
    responses(
        (status = 200, description = "Job created"),
        (status = 400, description = "Bad request"),
    ),
    security(("Bearer" = []))
)]
pub async fn start_detection(
    State(state): State<AppState>,
    Json(body): Json<StartDownloadDetectionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // All libraries case
    if body.library_id.is_none() {
        prowlarr::check_prowlarr_configured(&state.pool).await?;
        let library_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM libraries ORDER BY name"
        )
        .fetch_all(&state.pool)
        .await?;
        let mut last_job_id: Option<Uuid> = None;
        for library_id in library_ids {
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'download_detection' AND status IN ('pending', 'running') LIMIT 1",
            )
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?;
            if existing.is_some() { continue; }
            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'running', NOW())",
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
                if let Err(e) = process_download_detection(&pool, job_id, library_id).await {
                    warn!("[DOWNLOAD_DETECTION] job {job_id} failed: {e}");
                    let _ = sqlx::query(
                        "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
                    )
                    .bind(job_id)
                    .bind(e.to_string())
                    .execute(&pool)
                    .await;
                    notifications::notify(
                        pool,
                        notifications::NotificationEvent::DownloadDetectionFailed {
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
    sqlx::query("SELECT id FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("library not found"))?;

    // Verify Prowlarr is configured
    prowlarr::check_prowlarr_configured(&state.pool).await?;

    // Check no existing running job for this library
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'download_detection' AND status IN ('pending', 'running') LIMIT 1",
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
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'running', NOW())",
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
        if let Err(e) = process_download_detection(&pool, job_id, library_id).await {
            warn!("[DOWNLOAD_DETECTION] job {job_id} failed: {e}");
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .bind(e.to_string())
            .execute(&pool)
            .await;
            notifications::notify(
                pool,
                notifications::NotificationEvent::DownloadDetectionFailed {
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
// GET /download-detection/:id/report
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/download-detection/{id}/report",
    tag = "download_detection",
    params(("id" = String, Path, description = "Job UUID")),
    responses(
        (status = 200, body = DownloadDetectionReportDto),
        (status = 404, description = "Job not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_detection_report(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<Uuid>,
) -> Result<Json<DownloadDetectionReportDto>, ApiError> {
    let row = sqlx::query(
        "SELECT status, total_files FROM index_jobs WHERE id = $1 AND type = 'download_detection'",
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("job not found"))?;

    let job_status: String = row.get("status");
    let total_files: Option<i32> = row.get("total_files");

    let counts = sqlx::query(
        "SELECT status, COUNT(*) as cnt FROM download_detection_results WHERE job_id = $1 GROUP BY status",
    )
    .bind(job_id)
    .fetch_all(&state.pool)
    .await?;

    let mut found = 0i64;
    let mut not_found = 0i64;
    let mut no_missing = 0i64;
    let mut no_metadata = 0i64;
    let mut errors = 0i64;

    for r in &counts {
        let status: String = r.get("status");
        let cnt: i64 = r.get("cnt");
        match status.as_str() {
            "found" => found = cnt,
            "not_found" => not_found = cnt,
            "no_missing" => no_missing = cnt,
            "no_metadata" => no_metadata = cnt,
            "error" => errors = cnt,
            _ => {}
        }
    }

    Ok(Json(DownloadDetectionReportDto {
        job_id,
        status: job_status,
        total_series: total_files.unwrap_or(0) as i64,
        found,
        not_found,
        no_missing,
        no_metadata,
        errors,
    }))
}

// ---------------------------------------------------------------------------
// GET /download-detection/:id/results
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ResultsQuery {
    pub status: Option<String>,
}

#[utoipa::path(
    get,
    path = "/download-detection/{id}/results",
    tag = "download_detection",
    params(
        ("id" = String, Path, description = "Job UUID"),
        ("status" = Option<String>, Query, description = "Filter by status"),
    ),
    responses(
        (status = 200, body = Vec<DownloadDetectionResultDto>),
    ),
    security(("Bearer" = []))
)]
pub async fn get_detection_results(
    State(state): State<AppState>,
    axum::extract::Path(job_id): axum::extract::Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<ResultsQuery>,
) -> Result<Json<Vec<DownloadDetectionResultDto>>, ApiError> {
    let rows = if let Some(status_filter) = &query.status {
        sqlx::query(
            "SELECT ddr.id, ddr.series_id, COALESCE(s.name, 'unknown') AS series_name, ddr.status, ddr.missing_count, ddr.available_releases, ddr.error_message
             FROM download_detection_results ddr
             LEFT JOIN series s ON s.id = ddr.series_id
             WHERE ddr.job_id = $1 AND ddr.status = $2
             ORDER BY s.name",
        )
        .bind(job_id)
        .bind(status_filter)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT ddr.id, ddr.series_id, COALESCE(s.name, 'unknown') AS series_name, ddr.status, ddr.missing_count, ddr.available_releases, ddr.error_message
             FROM download_detection_results ddr
             LEFT JOIN series s ON s.id = ddr.series_id
             WHERE ddr.job_id = $1
             ORDER BY ddr.status, s.name",
        )
        .bind(job_id)
        .fetch_all(&state.pool)
        .await?
    };

    let results = rows
        .iter()
        .map(|row| {
            let releases_json: Option<serde_json::Value> = row.get("available_releases");
            let available_releases = releases_json.and_then(|v| {
                serde_json::from_value::<Vec<AvailableReleaseDto>>(v).ok()
            });
            DownloadDetectionResultDto {
                id: row.get("id"),
                series_id: row.get("series_id"),
                series_name: row.get("series_name"),
                status: row.get("status"),
                missing_count: row.get("missing_count"),
                available_releases,
                error_message: row.get("error_message"),
            }
        })
        .collect();

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// GET /download-detection/latest-found
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct LatestFoundPerLibraryDto {
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub library_name: String,
    pub results: Vec<AvailableDownloadDto>,
}

#[derive(Serialize, ToSchema)]
pub struct AvailableDownloadDto {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub series_id: Uuid,
    pub series_name: String,
    pub missing_count: i32,
    pub available_releases: Option<Vec<AvailableReleaseDto>>,
    pub updated_at: String,
    pub failed_download_count: i64,
}

/// Returns available downloads per library from the `available_downloads` table.
#[utoipa::path(
    get,
    path = "/download-detection/latest-found",
    tag = "download_detection",
    responses(
        (status = 200, body = Vec<LatestFoundPerLibraryDto>),
    ),
    security(("Bearer" = []))
)]
pub async fn get_latest_found(
    State(state): State<AppState>,
) -> Result<Json<Vec<LatestFoundPerLibraryDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT ad.id, ad.library_id, s.name AS series_name, ad.series_id, ad.missing_count, ad.available_releases, ad.updated_at, \
                l.name as library_name, \
                COALESCE(td_err.failed_count, 0) AS failed_download_count, \
                COALESCE(td_err.failed_volumes, ARRAY[]::integer[]) AS failed_volumes \
         FROM available_downloads ad \
         JOIN libraries l ON l.id = ad.library_id \
         JOIN series s ON s.id = ad.series_id \
         LEFT JOIN LATERAL ( \
             SELECT COUNT(*) AS failed_count, \
                    array_agg(DISTINCT vol) FILTER (WHERE vol IS NOT NULL) AS failed_volumes \
             FROM torrent_downloads td, unnest(td.expected_volumes) AS vol \
             WHERE td.library_id = ad.library_id \
               AND LOWER(td.series_name) = LOWER(s.name) \
               AND td.status = 'error' \
         ) td_err ON TRUE \
         ORDER BY l.name, s.name",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut libs: std::collections::BTreeMap<Uuid, LatestFoundPerLibraryDto> = std::collections::BTreeMap::new();

    for row in &rows {
        let library_id: Uuid = row.get("library_id");
        let updated_at: chrono::DateTime<chrono::Utc> = row.get("updated_at");
        let releases_json: Option<serde_json::Value> = row.get("available_releases");
        let failed_volumes: Vec<i32> = row.get("failed_volumes");
        let available_releases = releases_json.and_then(|v| {
            serde_json::from_value::<Vec<AvailableReleaseDto>>(v).ok()
        }).map(|releases| {
            releases.into_iter().map(|mut r| {
                // Mark release as previously failed if any of its matched volumes overlap with failed volumes
                if !failed_volumes.is_empty() {
                    r.has_failed = r.matched_missing_volumes.iter().any(|v| failed_volumes.contains(v));
                }
                r
            }).collect()
        });

        let entry = libs.entry(library_id).or_insert_with(|| LatestFoundPerLibraryDto {
            library_id,
            library_name: row.get("library_name"),
            results: Vec::new(),
        });

        entry.results.push(AvailableDownloadDto {
            id: row.get("id"),
            series_id: row.get("series_id"),
            series_name: row.get("series_name"),
            missing_count: row.get("missing_count"),
            available_releases,
            updated_at: updated_at.to_rfc3339(),
            failed_download_count: row.get("failed_download_count"),
        });
    }

    Ok(Json(libs.into_values().collect()))
}

/// Delete an available download entry, or a single release within it.
///
/// - Without `?release=N`: deletes the entire series entry.
/// - With `?release=N`: removes release at index N from the array;
///   if the array becomes empty, the entire entry is deleted.
#[utoipa::path(
    delete,
    path = "/available-downloads/{id}",
    tag = "download_detection",
    params(
        ("id" = String, Path, description = "Available download ID"),
        ("release" = Option<usize>, Query, description = "Release index to remove (omit to delete entire entry)"),
    ),
    responses(
        (status = 200, description = "Deleted"),
        (status = 404, description = "Not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_available_download(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<DeleteAvailableQuery>,
) -> Result<Json<crate::responses::OkResponse>, ApiError> {
    if let Some(release_idx) = query.release {
        // Remove a single release from the JSON array
        let row = sqlx::query("SELECT available_releases FROM available_downloads WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("available download not found"))?;

        let releases_json: Option<serde_json::Value> = row.get("available_releases");
        if let Some(serde_json::Value::Array(mut releases)) = releases_json {
            if release_idx >= releases.len() {
                return Err(ApiError::bad_request("release index out of bounds"));
            }
            releases.remove(release_idx);

            if releases.is_empty() {
                sqlx::query("DELETE FROM available_downloads WHERE id = $1")
                    .bind(id)
                    .execute(&state.pool)
                    .await?;
            } else {
                sqlx::query(
                    "UPDATE available_downloads SET available_releases = $1, updated_at = NOW() WHERE id = $2",
                )
                .bind(serde_json::Value::Array(releases))
                .bind(id)
                .execute(&state.pool)
                .await?;
            }
        } else {
            return Err(ApiError::not_found("no releases found"));
        }
    } else {
        // Delete the entire entry
        let result = sqlx::query("DELETE FROM available_downloads WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(ApiError::not_found("available download not found"));
        }
    }

    Ok(Json(crate::responses::OkResponse::new()))
}

#[derive(Deserialize)]
pub struct DeleteAvailableQuery {
    pub release: Option<usize>,
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

pub(crate) async fn process_download_detection(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(i32, i64), String> {
    let (prowlarr_url, prowlarr_api_key, categories) =
        prowlarr::load_prowlarr_config_internal(pool)
            .await
            .map_err(|e| e.message)?;

    // Fetch all series with their metadata link status
    let all_series_rows: Vec<(String, Option<uuid::Uuid>)> = sqlx::query_as(
        r#"
        SELECT DISTINCT COALESCE(s.name, 'unclassified') AS name, b.series_id
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

    // Clean up available_downloads for series that no longer exist
    sqlx::query(
        r#"
        DELETE FROM available_downloads
        WHERE library_id = $1
          AND series_id NOT IN (
            SELECT id FROM series WHERE library_id = $1
          )
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let all_series: Vec<String> = all_series_rows.iter().map(|(name, _)| name.clone()).collect();
    let series_id_map: std::collections::HashMap<String, Uuid> = all_series_rows.iter()
        .filter_map(|(name, id)| id.map(|id| (name.clone(), id)))
        .collect();
    let total = all_series.len() as i32;
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Fetch approved metadata links for this library (series_name -> link_id)
    let links: Vec<(String, Uuid)> = sqlx::query(
        "SELECT s.name AS series_name, eml.id FROM external_metadata_links eml \
         JOIN series s ON s.id = eml.series_id \
         WHERE eml.library_id = $1 AND eml.status = 'approved'",
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(|row| {
        let series_name: String = row.get("series_name");
        let link_id: Uuid = row.get("id");
        (series_name, link_id)
    })
    .collect();

    let link_map: std::collections::HashMap<String, Uuid> = links.into_iter().collect();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Stripstream-Librarian")
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let mut processed = 0i32;

    for series_name in &all_series {
        if is_job_cancelled(pool, job_id).await {
            sqlx::query(
                "UPDATE index_jobs SET status = 'cancelled', finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            return Ok((total, 0));
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

        // Skip unclassified
        if series_name == "unclassified" {
            insert_result(pool, job_id, library_id, series_id_map.get(series_name).copied(), "no_metadata", 0, None, None).await;
            continue;
        }

        // Check if this series has an approved metadata link
        let link_id = match link_map.get(series_name) {
            Some(id) => *id,
            None => {
                insert_result(pool, job_id, library_id, series_id_map.get(series_name).copied(), "no_metadata", 0, None, None).await;
                continue;
            }
        };

        // Fetch missing books for this series
        let missing_rows = sqlx::query(
            "SELECT volume_number FROM external_book_metadata WHERE link_id = $1 AND book_id IS NULL ORDER BY volume_number NULLS LAST",
        )
        .bind(link_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        if missing_rows.is_empty() {
            insert_result(pool, job_id, library_id, series_id_map.get(series_name).copied(), "no_missing", 0, None, None).await;
            // Series is complete, remove from available_downloads
            if let Some(&sid) = series_id_map.get(series_name) {
                let _ = sqlx::query("DELETE FROM available_downloads WHERE series_id = $1")
                    .bind(sid).execute(pool).await;
            }
            continue;
        }

        let missing_volumes: Vec<i32> = missing_rows
            .iter()
            .filter_map(|row| row.get::<Option<i32>, _>("volume_number"))
            .collect();
        let missing_count = missing_rows.len() as i32;

        // Search Prowlarr
        match search_prowlarr_for_series(
            &client,
            &prowlarr_url,
            &prowlarr_api_key,
            &categories,
            series_name,
            &missing_volumes,
        )
        .await
        {
            Ok(matched_releases) if !matched_releases.is_empty() => {
                let releases_json = serde_json::to_value(&matched_releases).ok();
                insert_result(
                    pool,
                    job_id,
                    library_id,
                    series_id_map.get(series_name).copied(),
                    "found",
                    missing_count,
                    releases_json.clone(),
                    None,
                )
                .await;
                // UPSERT into available_downloads — merge new releases with existing ones
                if let (Some(ref rj), Some(&sid)) = (&releases_json, series_id_map.get(series_name)) {
                    let _ = sqlx::query(
                        "INSERT INTO available_downloads (library_id, series_id, missing_count, available_releases, updated_at) \
                         VALUES ($1, $2, $3, $4, NOW()) \
                         ON CONFLICT (series_id) DO UPDATE SET \
                           missing_count = EXCLUDED.missing_count, \
                           available_releases = merge_releases(available_downloads.available_releases, EXCLUDED.available_releases), \
                           updated_at = NOW()",
                    )
                    .bind(library_id)
                    .bind(sid)
                    .bind(missing_count)
                    .bind(rj)
                    .execute(pool)
                    .await;
                }
            }
            Ok(_) => {
                insert_result(pool, job_id, library_id, series_id_map.get(series_name).copied(), "not_found", missing_count, None, None).await;
                // Don't delete — keep previous results even if this run found nothing
                // Only update missing_count
                if let Some(&sid) = series_id_map.get(series_name) {
                let _ = sqlx::query(
                    "UPDATE available_downloads SET missing_count = $2, updated_at = NOW() WHERE series_id = $1",
                )
                .bind(sid)
                .bind(missing_count)
                .execute(pool)
                .await;
                }
            }
            Err(e) => {
                warn!("[DOWNLOAD_DETECTION] series '{series_name}': {e}");
                insert_result(pool, job_id, library_id, series_id_map.get(series_name).copied(), "error", missing_count, None, Some(&e)).await;
            }
        }
    }

    // Build final stats
    let counts = sqlx::query(
        "SELECT status, COUNT(*) as cnt FROM download_detection_results WHERE job_id = $1 GROUP BY status",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut count_found = 0i64;
    let mut count_not_found = 0i64;
    let mut count_no_missing = 0i64;
    let mut count_no_metadata = 0i64;
    let mut count_errors = 0i64;
    for row in &counts {
        let s: String = row.get("status");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "found" => count_found = c,
            "not_found" => count_not_found = c,
            "no_missing" => count_no_missing = c,
            "no_metadata" => count_no_metadata = c,
            "error" => count_errors = c,
            _ => {}
        }
    }

    let stats = serde_json::json!({
        "total_series": total as i64,
        "found": count_found,
        "not_found": count_not_found,
        "no_missing": count_no_missing,
        "no_metadata": count_no_metadata,
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
        "[DOWNLOAD_DETECTION] job={job_id} completed: {total} series, found={count_found}, not_found={count_not_found}, no_missing={count_no_missing}, no_metadata={count_no_metadata}, errors={count_errors}"
    );

    let library_name: Option<String> = sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    notifications::notify(
        pool.clone(),
        notifications::NotificationEvent::DownloadDetectionCompleted {
            library_name,
            total_series: total,
            found: count_found,
        },
    );

    Ok((total, count_found))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn search_prowlarr_for_series(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    categories: &[i32],
    series_name: &str,
    missing_volumes: &[i32],
) -> Result<Vec<AvailableReleaseDto>, String> {
    let query = format!("\"{}\"", series_name);

    let mut params: Vec<(&str, String)> = vec![
        ("query", query),
        ("type", "search".to_string()),
    ];
    for cat in categories {
        params.push(("categories", cat.to_string()));
    }

    let resp = client
        .get(format!("{url}/api/v1/search"))
        .query(&params)
        .header("X-Api-Key", api_key)
        .send()
        .await
        .map_err(|e| format!("Prowlarr request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Prowlarr returned {status}: {text}"));
    }

    let raw_releases: Vec<prowlarr::ProwlarrRawRelease> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Prowlarr response: {e}"))?;

    let matched: Vec<AvailableReleaseDto> = raw_releases
        .into_iter()
        .filter_map(|r| {
            let (matched_vols, all_volumes) = prowlarr::match_title_volumes(&r.title, missing_volumes);

            if matched_vols.is_empty() {
                None
            } else {
                Some(AvailableReleaseDto {
                    title: r.title,
                    size: r.size,
                    download_url: r.download_url,
                    indexer: r.indexer,
                    seeders: r.seeders,
                    matched_missing_volumes: matched_vols,
                    all_volumes,
                    has_failed: false,
                })
            }
        })
        .collect();

    Ok(matched)
}

#[allow(clippy::too_many_arguments)]
async fn insert_result(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
    series_id: Option<Uuid>,
    status: &str,
    missing_count: i32,
    available_releases: Option<serde_json::Value>,
    error_message: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO download_detection_results
            (job_id, library_id, series_id, status, missing_count, available_releases, error_message)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(job_id)
    .bind(library_id)
    .bind(series_id)
    .bind(status)
    .bind(missing_count)
    .bind(&available_releases)
    .bind(error_message)
    .execute(pool)
    .await;
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
    use super::*;
    use sqlx::Row;

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn failed_download_count_query(pool: sqlx::PgPool) {
        // Setup: library + series
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Test Lib', '/libraries/test')")
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

        // Insert an available_download for this series
        let ad_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO available_downloads (id, library_id, series_id, missing_count, available_releases, updated_at) \
             VALUES ($1, $2, $3, 5, '[]'::jsonb, NOW())",
        )
        .bind(ad_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert torrent_downloads with status='error' matching the series name (case-insensitive)
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status, error_message) \
             VALUES ($1, $2, 'naruto', '{1,2}', 'error', 'stalled')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status, error_message) \
             VALUES ($1, $2, 'Naruto', '{3}', 'error', 'timeout')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // Also insert a non-error torrent to verify it's NOT counted
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status) \
             VALUES ($1, $2, 'Naruto', '{4}', 'imported')",
        )
        .bind(Uuid::new_v4())
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // Execute: run the same query used in get_latest_found
        let rows = sqlx::query(
            "SELECT ad.id, ad.library_id, s.name AS series_name, ad.series_id, ad.missing_count, ad.available_releases, ad.updated_at, \
                    l.name as library_name, \
                    COALESCE(td_err.failed_count, 0) AS failed_download_count \
             FROM available_downloads ad \
             JOIN libraries l ON l.id = ad.library_id \
             JOIN series s ON s.id = ad.series_id \
             LEFT JOIN LATERAL ( \
                 SELECT COUNT(*) AS failed_count \
                 FROM torrent_downloads td \
                 WHERE td.library_id = ad.library_id \
                   AND LOWER(td.series_name) = LOWER(s.name) \
                   AND td.status = 'error' \
             ) td_err ON TRUE \
             ORDER BY l.name, s.name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        // Assert
        assert_eq!(rows.len(), 1, "should return one available_download row");
        let row = &rows[0];
        let failed_count: i64 = row.get("failed_download_count");
        assert_eq!(failed_count, 2, "should count only the 2 error torrent_downloads, not the imported one");
        let series_name: String = row.get("series_name");
        assert_eq!(series_name, "Naruto");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn has_failed_flag_on_releases(pool: sqlx::PgPool) {
        // Setup: library + series
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Test', '/libraries/test')")
            .bind(library_id).execute(&pool).await.unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Blacksad')")
            .bind(series_id).bind(library_id).execute(&pool).await.unwrap();

        // Two releases: one matching failed volumes, one not
        let releases = serde_json::json!([
            { "title": "Blacksad T01-T03", "size": 100, "seeders": 10, "matched_missing_volumes": [1, 2, 3], "all_volumes": [1, 2, 3] },
            { "title": "Blacksad T05", "size": 50, "seeders": 5, "matched_missing_volumes": [5], "all_volumes": [5] }
        ]);

        sqlx::query(
            "INSERT INTO available_downloads (id, library_id, series_id, missing_count, available_releases, updated_at) \
             VALUES ($1, $2, $3, 5, $4, NOW())",
        )
        .bind(Uuid::new_v4()).bind(library_id).bind(series_id).bind(&releases)
        .execute(&pool).await.unwrap();

        // Failed torrent had volumes 2, 3 — overlaps with release 1 but not release 2
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, expected_volumes, status, error_message) \
             VALUES ($1, $2, 'Blacksad', '{2,3}', 'error', 'stalled')",
        )
        .bind(Uuid::new_v4()).bind(library_id)
        .execute(&pool).await.unwrap();

        // Run the actual query from get_latest_found
        let rows = sqlx::query(
            "SELECT ad.available_releases, \
                    COALESCE(td_err.failed_volumes, ARRAY[]::integer[]) AS failed_volumes \
             FROM available_downloads ad \
             JOIN series s ON s.id = ad.series_id \
             LEFT JOIN LATERAL ( \
                 SELECT array_agg(DISTINCT vol) FILTER (WHERE vol IS NOT NULL) AS failed_volumes \
                 FROM torrent_downloads td, unnest(td.expected_volumes) AS vol \
                 WHERE td.library_id = ad.library_id \
                   AND LOWER(td.series_name) = LOWER(s.name) \
                   AND td.status = 'error' \
             ) td_err ON TRUE \
             WHERE ad.series_id = $1",
        )
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        let failed_volumes: Vec<i32> = rows.get("failed_volumes");
        assert!(failed_volumes.contains(&2));
        assert!(failed_volumes.contains(&3));
        assert!(!failed_volumes.contains(&5), "volume 5 was not in a failed torrent");

        // Simulate the Rust-side enrichment
        let releases_json: serde_json::Value = rows.get("available_releases");
        let mut releases: Vec<AvailableReleaseDto> = serde_json::from_value(releases_json).unwrap();
        for r in &mut releases {
            r.has_failed = r.matched_missing_volumes.iter().any(|v| failed_volumes.contains(v));
        }

        assert!(releases[0].has_failed, "release T01-T03 should be flagged (volumes 2,3 overlap)");
        assert!(!releases[1].has_failed, "release T05 should NOT be flagged (volume 5 not failed)");
    }

    /// Regression: series_id must be returned from the detection results query.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_id_returned_when_series_exists(pool: sqlx::PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'OnePiece')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO download_detection_results (job_id, library_id, series_id, status, missing_count) \
             VALUES ($1, $2, $3, 'found', 3)",
        )
        .bind(job_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        // Run the actual query from get_detection_results (no status filter)
        let rows = sqlx::query(
            "SELECT ddr.id, ddr.series_id, COALESCE(s.name, 'unknown') AS series_name, ddr.status, ddr.missing_count, ddr.available_releases, ddr.error_message
             FROM download_detection_results ddr
             LEFT JOIN series s ON s.id = ddr.series_id
             WHERE ddr.job_id = $1
             ORDER BY ddr.status, s.name",
        )
        .bind(job_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        let returned_series_id: Option<Uuid> = rows[0].get("series_id");
        assert_eq!(returned_series_id, Some(series_id), "series_id should match the created series");
    }

    /// Regression: series_id should be None when series_id column is NULL.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_id_none_when_series_missing(pool: sqlx::PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'TestLib', '/libraries/test')")
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'download_detection', 'success', NOW())",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        // Insert with series_id = NULL
        sqlx::query(
            "INSERT INTO download_detection_results (job_id, library_id, series_id, status, missing_count) \
             VALUES ($1, $2, NULL, 'not_found', 0)",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let rows = sqlx::query(
            "SELECT ddr.id, ddr.series_id, COALESCE(s.name, 'unknown') AS series_name, ddr.status, ddr.missing_count, ddr.available_releases, ddr.error_message
             FROM download_detection_results ddr
             LEFT JOIN series s ON s.id = ddr.series_id
             WHERE ddr.job_id = $1
             ORDER BY ddr.status, s.name",
        )
        .bind(job_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        let returned_series_id: Option<Uuid> = rows[0].get("series_id");
        assert!(returned_series_id.is_none(), "series_id should be None when no series is linked");
    }
}
