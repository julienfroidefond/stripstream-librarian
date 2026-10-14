//! Background search across every enabled DDL provider.
//!
//! Mirrors `downloads::detection` but queries the DDL catalogue instead of
//! Prowlarr. Only series that carry an **approved metadata link** and still have
//! **missing regular volumes** are searched — the same perimeter the Telegram
//! monitor and the Prowlarr detection use.
//!
//! Discovered files are merged into `available_downloads`, the table the Prowlarr
//! detection and the RSS poll already write to, so they surface in the existing
//! "Disponibles" card of the downloads page. DDL releases carry `provider` and
//! `external_id`, letting the UI start a direct download.
//!
//! The run itself is *results-only*: it never downloads automatically.
//!
//! Each run refreshes the DDL subset of a series: previously stored DDL releases
//! (identified by a `provider` field) are stripped before the fresh results are
//! merged, while Prowlarr/Telegram-sourced releases are preserved.

use std::collections::HashSet;

use serde_json::json;
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::downloads::blacklist;
use crate::downloads::detection::AvailableReleaseDto;
use crate::downloads::missing;
use crate::downloads::store;
use crate::jobs::helpers::{insert_event, is_job_cancelled, update_progress};
use crate::jobs::lifecycle::complete_job;

use super::service;
use super::types::DdlProviderSearchResultDto;
use super::{enabled_providers, load_config};

/// Max files requested from each provider per series.
const PER_SERIES_LIMIT: usize = 50;

/// Process a `ddl_search` job: search every enabled provider for the missing
/// volumes of every series with an approved metadata link, then store the
/// matches in `available_downloads`.
pub async fn process_ddl_search(pool: &PgPool, job_id: Uuid) -> Result<(), String> {
    let config = load_config(pool).await.map_err(|e| e.message)?;
    let providers = enabled_providers(&config);

    if providers.is_empty() {
        info!("[DDL_SEARCH] No enabled provider, job {job_id} is a no-op");
        return complete_job(
            pool,
            job_id,
            json!({
                "series": 0,
                "series_with_results": 0,
                "files": 0,
                "providers": 0,
                "series_without_missing_volumes": 0,
                "series_with_missing_volumes": 0,
                "files_owned": 0,
                "files_other_series": 0,
                "files_no_volume_match": 0,
                "files_blacklisted": 0,
            }),
        )
        .await
        .map_err(|e| e.to_string());
    }

    let targets = missing::load_approved_links(pool, None, missing::LinkScope::All)
        .await
        .map_err(|e| e.to_string())?;

    // Titles the user blacklisted are never offered again, like detection/RSS.
    let blacklisted_titles = blacklist::load_blacklisted_titles(pool)
        .await
        .map_err(|e| e.to_string())?;

    let total = targets.len() as i32;
    let _ = sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await;

    info!(
        "[DDL_SEARCH] job {job_id}: {} series, {} provider(s)",
        total,
        providers.len()
    );

    let mut processed = 0i32;
    let mut matched_series = 0i64;
    let mut matched_files = 0i64;
    let mut errors = 0i64;
    // Per-stage funnel counters: they show *why* a series yields nothing, which
    // the aggregate `series_with_results` cannot tell apart.
    let mut series_without_missing_volumes = 0i64;
    let mut series_with_missing_volumes = 0i64;
    let mut files_owned = 0i64;
    let mut files_other_series = 0i64;
    let mut files_no_volume_match = 0i64;
    let mut files_blacklisted = 0i64;

    for target in targets {
        if is_job_cancelled(pool, job_id).await {
            let _ = sqlx::query(
                "UPDATE index_jobs SET status = 'cancelled', finished_at = NOW() WHERE id = $1",
            )
            .bind(job_id)
            .execute(pool)
            .await;
            return Ok(());
        }

        processed += 1;
        update_progress(pool, job_id, processed, total, &target.series_name).await;

        // Refresh the DDL subset for this series (Prowlarr/Telegram releases are
        // left untouched). Do it before the early-out so a series that became
        // complete also loses its stale DDL offers.
        strip_ddl_releases(pool, target.series_id).await?;

        let missing = missing::load_link_missing_volumes(pool, target.link_id)
            .await
            .map_err(|e| e.to_string())?;
        if missing.missing_volumes.is_empty() {
            series_without_missing_volumes += 1;
            continue;
        }
        series_with_missing_volumes += 1;

        let owned = service::owned_volumes(pool, target.library_id, &target.series_name)
            .await
            .map_err(|e| e.message)?;

        let mut releases: Vec<AvailableReleaseDto> = Vec::new();
        let mut seen_titles: HashSet<String> = HashSet::new();

        for (provider, base_url) in &providers {
            let meta = provider.meta();
            let files = match provider
                .search(base_url, &target.series_name, PER_SERIES_LIMIT)
                .await
            {
                Ok(files) => files,
                Err(err) => {
                    warn!(
                        "[DDL_SEARCH] {} search failed for '{}': {err:#}",
                        meta.id, target.series_name
                    );
                    errors += 1;
                    continue;
                }
            };

            let mut results: Vec<DdlProviderSearchResultDto> = files
                .iter()
                .map(|file| {
                    service::build_result(
                        meta.id,
                        file,
                        Some(&target.series_name),
                        &missing.missing_volumes,
                        &owned,
                    )
                })
                .collect();
            service::rank(&mut results);

            for result in results {
                if !result.series_matched {
                    files_other_series += 1;
                    continue;
                }
                if result.already_owned {
                    files_owned += 1;
                    continue;
                }
                if result.matched_missing_volumes.is_empty() {
                    files_no_volume_match += 1;
                    continue;
                }
                if blacklisted_titles.contains(&result.filename) {
                    files_blacklisted += 1;
                    continue;
                }
                // One offer per filename across providers (merge_releases also
                // dedups on `title`, which is the filename).
                if !seen_titles.insert(result.filename.clone()) {
                    continue;
                }
                releases.push(to_available_release(meta.id, &result));
            }
        }

        if releases.is_empty() {
            continue;
        }

        matched_series += 1;
        matched_files += releases.len() as i64;

        let detected_at = chrono::Utc::now().to_rfc3339();
        for release in &mut releases {
            release.detected_at = Some(detected_at.clone());
        }

        insert_event(
            pool,
            job_id,
            "ddl_downloads_found",
            "info",
            Some("series"),
            Some(&target.series_name),
            None,
            Some(json!({
                "release_count": releases.len(),
                "missing_count": missing.missing_count,
            })),
        )
        .await;

        store::upsert_releases(
            pool,
            target.library_id,
            target.series_id,
            missing.missing_count,
            &releases,
        )
        .await
        .map_err(|e| e.to_string())?;
    }

    // Rows emptied by the refresh step (a series that used to have DDL offers and
    // now matches nothing) are dropped, like the detection cleanup does.
    let _ = store::delete_empty(pool, None).await;

    complete_job(
        pool,
        job_id,
        json!({
            "series": processed,
            "series_with_results": matched_series,
            "files": matched_files,
            "providers": providers.len(),
            "errors": errors,
            "series_without_missing_volumes": series_without_missing_volumes,
            "series_with_missing_volumes": series_with_missing_volumes,
            "files_owned": files_owned,
            "files_other_series": files_other_series,
            "files_no_volume_match": files_no_volume_match,
            "files_blacklisted": files_blacklisted,
        }),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Remove the DDL-sourced releases (those carrying a `provider`) from a series'
/// stored offers, keeping every other source intact.
async fn strip_ddl_releases(pool: &PgPool, series_id: Uuid) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE available_downloads
        SET available_releases = COALESCE((
                SELECT jsonb_agg(release)
                FROM jsonb_array_elements(available_releases) AS release
                WHERE release->>'provider' IS NULL
            ), '[]'::jsonb),
            updated_at = NOW()
        WHERE series_id = $1
        "#,
    )
    .bind(series_id)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Map a ranked DDL search result to the shared release shape written to
/// `available_downloads`.
fn to_available_release(
    provider_id: &str,
    result: &DdlProviderSearchResultDto,
) -> AvailableReleaseDto {
    AvailableReleaseDto {
        title: result.filename.clone(),
        size: result.size.unwrap_or(0),
        download_url: None,
        indexer: None,
        seeders: None,
        matched_missing_volumes: result.matched_missing_volumes.clone(),
        all_volumes: result.all_volumes.clone(),
        has_failed: false,
        detected_at: None,
        match_confidence: result
            .match_confidence
            .clone()
            .unwrap_or_else(|| "review".to_string()),
        match_reasons: Vec::new(),
        provider: Some(provider_id.to_string()),
        external_id: Some(result.external_id.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    fn sample_result() -> DdlProviderSearchResultDto {
        DdlProviderSearchResultDto {
            provider: "fourtoutici".to_string(),
            external_id: "file-42".to_string(),
            filename: "Astérix - Tome 3.cbz".to_string(),
            size: Some(1234),
            series_name: Some("Astérix".to_string()),
            volume_number: Some(3),
            all_volumes: vec![3],
            matched_missing_volumes: vec![3],
            match_confidence: Some("high".to_string()),
            series_matched: true,
            already_owned: false,
            download_id: None,
            download_status: None,
        }
    }

    #[test]
    fn ddl_release_carries_provider_and_external_id() {
        let release = to_available_release("fourtoutici", &sample_result());

        assert_eq!(release.title, "Astérix - Tome 3.cbz");
        assert_eq!(release.size, 1234);
        assert!(release.download_url.is_none());
        assert!(release.indexer.is_none());
        assert_eq!(release.provider.as_deref(), Some("fourtoutici"));
        assert_eq!(release.external_id.as_deref(), Some("file-42"));
        assert_eq!(release.matched_missing_volumes, vec![3]);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn process_ddl_search_is_a_noop_without_enabled_provider(pool: sqlx::PgPool) {
        // The 0120 seed ships every provider disabled.
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'ddl_search', 'running')",
        )
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();

        process_ddl_search(&pool, job_id).await.unwrap();

        let row = sqlx::query("SELECT status, stats_json FROM index_jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("status"), "success");
        let stats: serde_json::Value = row.get("stats_json");
        assert_eq!(stats["providers"], 0);
        assert_eq!(stats["series_with_results"], 0);
        assert_eq!(stats["series_without_missing_volumes"], 0);
        assert_eq!(stats["files_other_series"], 0);
        assert_eq!(stats["files_no_volume_match"], 0);

        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM available_downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn strip_ddl_releases_keeps_other_sources(pool: sqlx::PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Lib', '/libraries/lib')",
        )
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Astérix')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO available_downloads \
                (library_id, series_id, missing_count, available_releases, updated_at) \
             VALUES ($1, $2, 2, $3, NOW())",
        )
        .bind(library_id)
        .bind(series_id)
        .bind(serde_json::json!([
            { "title": "Astérix T01 [prowlarr]", "size": 1 },
            { "title": "Astérix T02 [ddl]", "size": 2, "provider": "fourtoutici", "external_id": "x" }
        ]))
        .execute(&pool)
        .await
        .unwrap();

        strip_ddl_releases(&pool, series_id).await.unwrap();

        let stored: serde_json::Value = sqlx::query_scalar(
            "SELECT available_releases FROM available_downloads WHERE series_id = $1",
        )
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let releases = stored.as_array().unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0]["title"], "Astérix T01 [prowlarr]");
        assert!(releases[0].get("provider").is_none());
    }
}
