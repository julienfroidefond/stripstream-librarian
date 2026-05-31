use axum::{extract::State, Json};
use serde::Deserialize;
use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};
use super::{detection::{insert_event, AvailableReleaseDto}, prowlarr};

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
        let library_ids: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM libraries ORDER BY name")
                .fetch_all(&state.pool)
                .await?;

        let mut last_job_id: Option<Uuid> = None;
        for library_id in library_ids {
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM index_jobs WHERE library_id = $1 AND type = 'prowlarr_rss' AND status IN ('pending', 'running') LIMIT 1",
            )
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?;
            if existing.is_some() {
                continue;
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
                if let Err(e) = process_rss_poll(&pool, job_id, library_id).await {
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
        if let Err(e) = process_rss_poll(&pool, job_id, library_id).await {
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

pub(crate) async fn process_rss_poll(
    pool: &PgPool,
    job_id: Uuid,
    library_id: Uuid,
) -> Result<(), String> {
    let job_started_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT COALESCE(started_at, created_at) FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let (prowlarr_url, prowlarr_api_key, categories) =
        prowlarr::load_prowlarr_config_internal(pool)
            .await
            .map_err(|e| e.message)?;

    // Load all series in this library that have an approved metadata link with missing volumes
    let series_rows = sqlx::query(
        r#"
        SELECT s.id AS series_id, s.name AS series_name, eml.id AS link_id
        FROM series s
        JOIN external_metadata_links eml ON eml.series_id = s.id
            AND eml.library_id = $1 AND eml.status = 'approved'
        WHERE EXISTS (
            SELECT 1 FROM external_book_metadata ebm
            WHERE ebm.link_id = eml.id AND ebm.book_id IS NULL
        )
        ORDER BY s.name
        "#,
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

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

    // Build series info: (series_id, name, missing_volumes)
    let mut series_info: Vec<(Uuid, String, Vec<i32>)> = Vec::new();
    for row in &series_rows {
        let series_id: Uuid = row.get("series_id");
        let series_name: String = row.get("series_name");
        let link_id: Uuid = row.get("link_id");

        let missing_vols: Vec<i32> = sqlx::query_scalar(
            "SELECT volume_number FROM external_book_metadata WHERE link_id = $1 AND book_id IS NULL AND volume_number IS NOT NULL AND volume_number > 0 ORDER BY volume_number",
        )
        .bind(link_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        series_info.push((series_id, series_name, missing_vols));
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

    let rss_releases = fetch_rss_releases(&client, &prowlarr_url, &prowlarr_api_key, &categories).await?;

    info!("[RSS_POLL] job={job_id} fetched {} releases from Prowlarr", rss_releases.len());

    let now_str = chrono::Utc::now().to_rfc3339();

    for (series_id, series_name, missing_volumes) in &series_info {
        if missing_volumes.is_empty() {
            continue;
        }

        let missing_count = missing_volumes.len() as i32;

        let matched_releases: Vec<AvailableReleaseDto> = rss_releases
            .iter()
            .filter(|r| !blacklisted_titles.contains(&r.title))
            .filter(|r| title_matches_series(&r.title, series_name))
            .filter_map(|r| {
                let (matched_vols, all_vols) =
                    prowlarr::match_title_volumes(&r.title, missing_volumes);
                if matched_vols.is_empty() {
                    None
                } else {
                    Some(AvailableReleaseDto {
                        title: r.title.clone(),
                        size: r.size,
                        download_url: r.download_url.clone(),
                        indexer: r.indexer.clone(),
                        seeders: r.seeders,
                        matched_missing_volumes: matched_vols,
                        all_volumes: all_vols,
                        has_failed: false,
                        detected_at: Some(now_str.clone()),
                    })
                }
            })
            .collect();

        if matched_releases.is_empty() {
            insert_event(pool, job_id, "downloads_not_found", "info", Some(series_name), None,
                Some(serde_json::json!({"missing_count": missing_count}))).await;
            continue;
        }

        // Stamp with detected_at already done above; upsert into available_downloads
        let releases_json = serde_json::to_value(&matched_releases).ok();
        insert_event(pool, job_id, "downloads_found", "info", Some(series_name), None,
            Some(serde_json::json!({
                "release_count": matched_releases.len(),
                "missing_count": missing_count,
                "available_releases": releases_json,
            }))).await;

        if let Some(ref rj) = releases_json {
            let _ = sqlx::query(
                "INSERT INTO available_downloads (library_id, series_id, missing_count, available_releases, updated_at) \
                 VALUES ($1, $2, $3, $4, NOW()) \
                 ON CONFLICT (series_id) DO UPDATE SET \
                   missing_count = EXCLUDED.missing_count, \
                   available_releases = merge_releases(available_downloads.available_releases, EXCLUDED.available_releases), \
                   updated_at = NOW()",
            )
            .bind(library_id)
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

    let new_releases: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM available_downloads ad,
             jsonb_array_elements(ad.available_releases) AS rel
        WHERE ad.library_id = $1
          AND rel->>'detected_at' IS NOT NULL
          AND (rel->>'detected_at')::timestamptz >= $2
        "#,
    )
    .bind(library_id)
    .bind(job_started_at)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let stats = serde_json::json!({
        "total_series": total as i64,
        "found": count_found,
        "new_releases": new_releases,
        "rss_releases_fetched": rss_releases.len() as i64,
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
        "[RSS_POLL] job={job_id} completed: {total} series, found={count_found}, new_releases={new_releases}"
    );

    let library_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

    if new_releases > 0 {
        notifications::notify(
            pool.clone(),
            notifications::NotificationEvent::DownloadDetectionCompleted {
                library_name,
                total_series: total,
                found: count_found,
                new_releases,
                not_found: count_not_found,
                no_missing: 0,
                no_metadata: 0,
                errors: 0,
            },
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn fetch_rss_releases(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    categories: &[i32],
) -> Result<Vec<prowlarr::ProwlarrRawRelease>, String> {
    let mut params: Vec<(&str, String)> = vec![
        ("query", String::new()),
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
        .map_err(|e| format!("Prowlarr RSS request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Prowlarr returned {status}: {text}"));
    }

    resp.json::<Vec<prowlarr::ProwlarrRawRelease>>()
        .await
        .map_err(|e| format!("Failed to parse Prowlarr response: {e}"))
}

/// Normalize a string for fuzzy matching:
/// - dots, underscores, hyphens → space
/// - accented chars → ASCII equivalent (common French chars)
/// - lowercase
fn normalize_for_match(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '.' | '_' => ' ',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'ù' | 'û' | 'ü' => 'u',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ç' => 'c',
            'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'À' | 'Â' | 'Ä' => 'a',
            'Ù' | 'Û' | 'Ü' => 'u',
            'Î' | 'Ï' => 'i',
            'Ô' | 'Ö' => 'o',
            'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn title_matches_series(title: &str, series_name: &str) -> bool {
    let norm_title = normalize_for_match(title);
    let norm_name = normalize_for_match(series_name);
    norm_title.contains(&norm_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_matches_basic() {
        assert!(title_matches_series("Asterix et Obelix T01", "Asterix et Obelix"));
        assert!(title_matches_series("ASTERIX T01", "asterix"));
        assert!(!title_matches_series("One Piece T01", "Naruto"));
    }

    #[test]
    fn title_matches_dots_and_accents() {
        assert!(title_matches_series("Asterix.et.Obelix.T01.FRENCH.CBZ", "Astérix et Obélix"));
        assert!(title_matches_series("Les.Legendaires.T05.FRENCH", "Les Légendaires"));
        assert!(title_matches_series("One.Piece.Tome.25.FRENCH", "One Piece"));
        assert!(!title_matches_series("One.Piece.T01", "Dragon Ball"));
    }
}
