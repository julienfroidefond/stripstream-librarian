use axum::{extract::State, Json};
use parsers::match_release_title;
use serde::Deserialize;
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

use super::{
    detection::{insert_event, AvailableReleaseDto},
    missing, prowlarr,
};
use crate::{error::ApiError, state::AppState};

// ---------------------------------------------------------------------------
// POST /prowlarr-rss/start
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct StartRssPollRequest {
    pub library_id: Option<String>,
}

pub async fn start_rss_poll(
    State(state): State<AppState>,
    Json(body): Json<StartRssPollRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    prowlarr::check_prowlarr_configured(&state.pool).await?;

    if body.library_id.is_none() {
        // Global job: one RSS fetch covers all libraries
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM index_jobs WHERE library_id IS NULL AND type = 'prowlarr_rss' AND status IN ('pending', 'running') LIMIT 1",
        )
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
            "INSERT INTO index_jobs (id, type, status, started_at) VALUES ($1, 'prowlarr_rss', 'running', NOW())",
        )
        .bind(job_id)
        .execute(&state.pool)
        .await?;

        let pool = state.pool.clone();
        tokio::spawn(async move {
            if let Err(e) = process_rss_poll(&pool, job_id, None).await {
                tracing::warn!("[RSS_POLL] job {job_id} failed: {e}");
                let _ = sqlx::query(
                    "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
                )
                .bind(job_id)
                .bind(&e)
                .execute(&pool)
                .await;
            }
        });

        return Ok(Json(serde_json::json!({
            "id": job_id.to_string(),
            "status": "started",
        })));
    }

    // Per-library trigger (backward compat — still fetches RSS for just this library)
    let library_id: Uuid = body
        .library_id
        .unwrap()
        .parse()
        .map_err(|_| ApiError::bad_request("invalid library_id"))?;

    sqlx::query("SELECT id FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("library not found"))?;

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'prowlarr_rss' AND status IN ('pending', 'running') LIMIT 1",
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
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) VALUES ($1, $2, 'prowlarr_rss', 'running', NOW())",
    )
    .bind(job_id)
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Err(e) = process_rss_poll(&pool, job_id, Some(library_id)).await {
            tracing::warn!("[RSS_POLL] job {job_id} failed: {e}");
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'failed', error_opt = $2, finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .bind(&e)
            .execute(&pool)
            .await;
        }
    });

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "running",
    })))
}

// ---------------------------------------------------------------------------
// Background processing
// ---------------------------------------------------------------------------

/// Process a prowlarr_rss job.
/// When library_id is None, fetches the RSS feed once and matches against all
/// libraries — this is the normal scheduler path.
/// When library_id is Some, processes only that library (manual per-lib trigger).
pub(crate) async fn process_rss_poll(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Option<Uuid>,
) -> Result<(), String> {
    let job_started_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT COALESCE(started_at, created_at) FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    let (prowlarr_url, prowlarr_api_key, categories) =
        prowlarr::load_prowlarr_config_internal(pool)
            .await
            .map_err(|e| e.message)?;

    // Load all series that have an approved metadata link with missing volumes.
    // When library_id is None, covers every library (single RSS fetch).
    let series_rows = if let Some(lid) = library_id {
        sqlx::query(
            r#"
            SELECT s.id AS series_id, s.name AS series_name, eml.id AS link_id, eml.library_id AS library_id
            FROM series s
            JOIN external_metadata_links eml ON eml.series_id = s.id
                AND eml.library_id = $1 AND eml.status = 'approved'
            WHERE EXISTS (
                SELECT 1 FROM external_book_metadata ebm
                WHERE ebm.link_id = eml.id AND ebm.book_id IS NULL
            )
              AND NOT EXISTS (
                  SELECT 1 FROM books b
                  WHERE b.series_id = s.id AND b.volume_type = 'integral'
              )
            ORDER BY s.name
            "#,
        )
        .bind(lid)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query(
            r#"
            SELECT s.id AS series_id, s.name AS series_name, eml.id AS link_id, eml.library_id AS library_id
            FROM series s
            JOIN external_metadata_links eml ON eml.series_id = s.id AND eml.status = 'approved'
            WHERE EXISTS (
                SELECT 1 FROM external_book_metadata ebm
                WHERE ebm.link_id = eml.id AND ebm.book_id IS NULL
            )
              AND NOT EXISTS (
                  SELECT 1 FROM books b
                  WHERE b.series_id = s.id AND b.volume_type = 'integral'
              )
            ORDER BY s.name
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
    };

    if series_rows.is_empty() {
        sqlx::query(
            "UPDATE index_jobs SET status = 'success', finished_at = NOW(), stats_json = $2, progress_percent = 100 WHERE id = $1",
        )
        .bind(job_id)
        .bind(serde_json::json!({"message": "No series with missing volumes"}))
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let total = series_rows.len() as i32;
    sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Build series info: (series_id, name, missing_volumes, library_id)
    let mut series_info: Vec<(Uuid, String, Vec<i32>, Uuid)> = Vec::new();
    for row in &series_rows {
        let series_id: Uuid = row.get("series_id");
        let series_name: String = row.get("series_name");
        let link_id: Uuid = row.get("link_id");
        let lib_id: Uuid = row.get("library_id");

        let missing_vols = missing::load_link_missing_volumes(pool, link_id)
            .await
            .map(|missing| missing.missing_volumes)
            .unwrap_or_default();

        series_info.push((series_id, series_name, missing_vols, lib_id));
    }

    let blacklisted_titles: std::collections::HashSet<String> =
        sqlx::query_scalar("SELECT title FROM release_blacklist")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("Stripstream-Librarian")
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    // Single RSS fetch regardless of how many libraries/series are covered
    let RssFetchResult {
        releases: rss_releases,
        indexer_stats,
    } = fetch_rss_releases(&client, &prowlarr_url, &prowlarr_api_key, &categories).await?;

    info!(
        "[RSS_POLL] job={job_id} fetched {} releases from Prowlarr ({} indexers)",
        rss_releases.len(),
        indexer_stats.len()
    );

    let now_str = chrono::Utc::now().to_rfc3339();

    for (series_id, series_name, missing_volumes, lib_id) in &series_info {
        if missing_volumes.is_empty() {
            continue;
        }

        let missing_count = missing_volumes.len() as i32;

        let matched_releases: Vec<AvailableReleaseDto> = rss_releases
            .iter()
            .filter(|r| !blacklisted_titles.contains(&r.title))
            .filter_map(|r| {
                if let Some(matched) = match_release_title(&r.title, series_name, missing_volumes) {
                    Some(AvailableReleaseDto {
                        title: r.title.clone(),
                        size: r.size,
                        download_url: r.download_url.clone(),
                        indexer: r.indexer.clone(),
                        seeders: r.seeders,
                        matched_missing_volumes: matched.matched_missing_volumes,
                        all_volumes: matched.all_volumes,
                    has_failed: false,
                    detected_at: Some(now_str.clone()),
                    match_confidence: matched.confidence.as_str().to_string(),
                    match_reasons: matched
                        .reasons
                        .into_iter()
                        .map(|reason| reason.as_str().to_string())
                        .collect(),
                })
                } else {
                    None
                }
            })
            .collect();

        if matched_releases.is_empty() {
            insert_event(
                pool,
                job_id,
                "downloads_not_found",
                "info",
                Some(series_name),
                None,
                Some(serde_json::json!({"missing_count": missing_count})),
            )
            .await;
            continue;
        }

        let releases_json = serde_json::to_value(&matched_releases).ok();
        insert_event(
            pool,
            job_id,
            "downloads_found",
            "info",
            Some(series_name),
            None,
            Some(serde_json::json!({
                "release_count": matched_releases.len(),
                "missing_count": missing_count,
                "available_releases": releases_json,
            })),
        )
        .await;

        if let Some(ref rj) = releases_json {
            let _ = sqlx::query(
                "INSERT INTO available_downloads (library_id, series_id, missing_count, available_releases, updated_at) \
                 VALUES ($1, $2, $3, $4, NOW()) \
                 ON CONFLICT (series_id) DO UPDATE SET \
                   missing_count = EXCLUDED.missing_count, \
                   available_releases = merge_releases(available_downloads.available_releases, EXCLUDED.available_releases), \
                   updated_at = NOW()",
            )
            .bind(lib_id)
            .bind(series_id)
            .bind(missing_count)
            .bind(rj)
            .execute(pool)
            .await;
        }
    }

    // Count from events
    let event_counts = sqlx::query(
        "SELECT event_type, COUNT(*) as cnt FROM index_job_events WHERE job_id = $1 GROUP BY event_type",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut count_found = 0i64;
    let mut count_not_found = 0i64;
    for row in &event_counts {
        let s: String = row.get("event_type");
        let c: i64 = row.get("cnt");
        match s.as_str() {
            "downloads_found" => count_found = c,
            "downloads_not_found" => count_not_found = c,
            _ => {}
        }
    }

    // Count new releases detected during this run
    let new_releases: i64 = if let Some(lid) = library_id {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM available_downloads ad,
                 jsonb_array_elements(ad.available_releases) AS rel
            WHERE ad.library_id = $1
              AND rel->>'detected_at' IS NOT NULL
              AND (rel->>'detected_at')::timestamptz >= $2
            "#,
        )
        .bind(lid)
        .bind(job_started_at)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM available_downloads ad,
                 jsonb_array_elements(ad.available_releases) AS rel
            WHERE rel->>'detected_at' IS NOT NULL
              AND (rel->>'detected_at')::timestamptz >= $1
            "#,
        )
        .bind(job_started_at)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
    };

    // Cap snapshot at 200 releases to bound storage per job
    let snapshot: Vec<_> = rss_releases
        .iter()
        .take(200)
        .map(|r| {
            serde_json::json!({
                "title": r.title,
                "indexer": r.indexer,
                "size": r.size,
                "seeders": r.seeders,
                "leechers": r.leechers,
                "publish_date": r.publish_date,
                "categories": r.categories.as_ref().map(|cats| cats.iter()
                    .filter_map(|c| c.name.as_deref())
                    .collect::<Vec<_>>()),
            })
        })
        .collect();

    let stats = serde_json::json!({
        "total_series": total as i64,
        "found": count_found,
        "new_releases": new_releases,
        "rss_releases_fetched": rss_releases.len() as i64,
        "rss_indexer_stats": indexer_stats,
        "rss_releases": snapshot,
    });

    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), stats_json = $2, progress_percent = 100 WHERE id = $1",
    )
    .bind(job_id)
    .bind(&stats)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // Keep rss_releases snapshot only for the 5 most recent successful jobs
    let _ = sqlx::query(
        r#"
        UPDATE index_jobs
        SET stats_json = stats_json - 'rss_releases'
        WHERE type = 'prowlarr_rss'
          AND id NOT IN (
            SELECT id FROM index_jobs
            WHERE type = 'prowlarr_rss' AND status = 'success'
            ORDER BY finished_at DESC
            LIMIT 5
          )
        "#,
    )
    .execute(pool)
    .await;

    info!(
        "[RSS_POLL] job={job_id} completed: {total} series, found={count_found}, new_releases={new_releases}"
    );

    if new_releases > 0 {
        let new_items: Vec<(String, String)> = if let Some(lid) = library_id {
            sqlx::query(
                r#"
                SELECT DISTINCT s.name AS series_name, rel->>'title' AS release_title
                FROM available_downloads ad
                CROSS JOIN jsonb_array_elements(ad.available_releases) AS rel
                JOIN series s ON s.id = ad.series_id
                WHERE ad.library_id = $1
                  AND (rel->>'detected_at')::timestamptz >= $2
                ORDER BY s.name
                LIMIT 10
                "#,
            )
            .bind(lid)
            .bind(job_started_at)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("series_name"),
                    r.get::<String, _>("release_title"),
                )
            })
            .collect()
        } else {
            sqlx::query(
                r#"
                SELECT DISTINCT s.name AS series_name, rel->>'title' AS release_title
                FROM available_downloads ad
                CROSS JOIN jsonb_array_elements(ad.available_releases) AS rel
                JOIN series s ON s.id = ad.series_id
                WHERE (rel->>'detected_at')::timestamptz >= $1
                ORDER BY s.name
                LIMIT 10
                "#,
            )
            .bind(job_started_at)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("series_name"),
                    r.get::<String, _>("release_title"),
                )
            })
            .collect()
        };

        notifications::notify(
            pool.clone(),
            notifications::NotificationEvent::DownloadDetectionCompleted {
                library_name: None,
                total_series: total,
                found: count_found,
                new_releases,
                not_found: count_not_found,
                no_missing: 0,
                no_metadata: 0,
                errors: 0,
                new_items,
            },
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) struct RssFetchResult {
    pub releases: Vec<prowlarr::ProwlarrRawRelease>,
    pub indexer_stats: Vec<serde_json::Value>,
}

/// Fetch recent releases from all Prowlarr indexers in parallel, aggregated
/// and deduplicated by GUID. Each indexer is queried independently so private
/// trackers that don't support empty-query browse don't suppress others.
async fn fetch_rss_releases(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    categories: &[i32],
) -> Result<RssFetchResult, String> {
    // Step 1: get the list of configured indexers
    let indexers_resp = client
        .get(format!("{url}/api/v1/indexer"))
        .header("X-Api-Key", api_key)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch indexer list: {e}"))?;

    let indexers: Vec<serde_json::Value> = if indexers_resp.status().is_success() {
        indexers_resp.json().await.unwrap_or_default()
    } else {
        vec![]
    };

    let indexer_meta: Vec<(i64, String)> = indexers
        .iter()
        .filter_map(|i| {
            let id = i["id"].as_i64()?;
            let name = i["name"].as_str().unwrap_or("unknown").to_string();
            Some((id, name))
        })
        .collect();

    // Step 2: fire one request per indexer in parallel
    let tasks: Vec<_> = indexer_meta
        .iter()
        .map(|(id, name)| {
            let client = client.clone();
            let url = url.to_string();
            let api_key = api_key.to_string();
            let categories = categories.to_vec();
            let indexer_id = *id;
            let indexer_name = name.clone();
            async move {
                let result =
                    fetch_rss_for_indexer(&client, &url, &api_key, &categories, indexer_id).await;
                (indexer_id, indexer_name, result)
            }
        })
        .collect();

    let results = futures::future::join_all(tasks).await;

    // Step 3: aggregate + deduplicate by GUID, collect per-indexer stats
    let mut seen = std::collections::HashSet::new();
    let mut all = Vec::new();
    let mut indexer_stats = Vec::new();

    for (id, name, result) in results {
        match result {
            Ok(releases) => {
                let count = releases.len();
                let mut deduped = 0usize;
                for r in releases {
                    if seen.insert(r.guid.clone()) {
                        all.push(r);
                        deduped += 1;
                    }
                }
                indexer_stats.push(serde_json::json!({
                    "id": id, "name": name, "count": count, "deduped": deduped
                }));
            }
            Err(e) => {
                tracing::debug!("[RSS_POLL] indexer {name} ({id}) fetch failed: {e}");
                indexer_stats.push(serde_json::json!({
                    "id": id, "name": name, "count": 0, "error": e
                }));
            }
        }
    }

    Ok(RssFetchResult {
        releases: all,
        indexer_stats,
    })
}

async fn fetch_rss_for_indexer(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    categories: &[i32],
    indexer_id: i64,
) -> Result<Vec<prowlarr::ProwlarrRawRelease>, String> {
    let mut params: Vec<(&str, String)> = vec![
        ("query", String::new()),
        ("type", "search".to_string()),
        ("limit", "100".to_string()),
        ("indexerIds", indexer_id.to_string()),
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
        .map_err(|e| format!("Prowlarr RSS request failed for indexer {indexer_id}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Prowlarr returned {status} for indexer {indexer_id}: {text}"
        ));
    }

    resp.json::<Vec<prowlarr::ProwlarrRawRelease>>()
        .await
        .map_err(|e| format!("Failed to parse Prowlarr response for indexer {indexer_id}: {e}"))
}
