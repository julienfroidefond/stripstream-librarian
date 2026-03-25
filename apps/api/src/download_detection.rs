use axum::{extract::State, Json};
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
    pub library_id: String,
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
    let library_id: Uuid = body
        .library_id
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
        match process_download_detection(&pool, job_id, library_id).await {
            Ok((total_series, found)) => {
                notifications::notify(
                    pool,
                    notifications::NotificationEvent::DownloadDetectionCompleted {
                        library_name,
                        total_series,
                        found,
                    },
                );
            }
            Err(e) => {
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
            "SELECT id, series_name, status, missing_count, available_releases, error_message
             FROM download_detection_results
             WHERE job_id = $1 AND status = $2
             ORDER BY series_name",
        )
        .bind(job_id)
        .bind(status_filter)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, series_name, status, missing_count, available_releases, error_message
             FROM download_detection_results
             WHERE job_id = $1
             ORDER BY status, series_name",
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
    let all_series: Vec<String> = sqlx::query_scalar(
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

    let total = all_series.len() as i32;
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Fetch approved metadata links for this library (series_name -> link_id)
    let links: Vec<(String, Uuid)> = sqlx::query(
        "SELECT series_name, id FROM external_metadata_links WHERE library_id = $1 AND status = 'approved'",
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
            insert_result(pool, job_id, library_id, series_name, "no_metadata", 0, None, None).await;
            continue;
        }

        // Check if this series has an approved metadata link
        let link_id = match link_map.get(series_name) {
            Some(id) => *id,
            None => {
                insert_result(pool, job_id, library_id, series_name, "no_metadata", 0, None, None).await;
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
            insert_result(pool, job_id, library_id, series_name, "no_missing", 0, None, None).await;
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
                    series_name,
                    "found",
                    missing_count,
                    releases_json,
                    None,
                )
                .await;
            }
            Ok(_) => {
                insert_result(pool, job_id, library_id, series_name, "not_found", missing_count, None, None).await;
            }
            Err(e) => {
                warn!("[DOWNLOAD_DETECTION] series '{series_name}': {e}");
                insert_result(pool, job_id, library_id, series_name, "error", missing_count, None, Some(&e)).await;
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
            let title_volumes = prowlarr::extract_volumes_from_title_pub(&r.title);
            let matched_vols: Vec<i32> = title_volumes
                .into_iter()
                .filter(|v| missing_volumes.contains(v))
                .collect();
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
    series_name: &str,
    status: &str,
    missing_count: i32,
    available_releases: Option<serde_json::Value>,
    error_message: Option<&str>,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO download_detection_results
            (job_id, library_id, series_name, status, missing_count, available_releases, error_message)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(job_id)
    .bind(library_id)
    .bind(series_name)
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
