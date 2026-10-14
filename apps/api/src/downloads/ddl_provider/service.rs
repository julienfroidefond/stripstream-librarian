//! DDL provider search orchestration and the download → import worker.
//!
//! Two flows live here:
//!
//! * [`search`] — stateless catalogue search. Results are ranked against the
//!   target series' missing volumes and flagged when the volume is already owned.
//! * [`start_ddl_download`] — persists a `ddl_provider_downloads` row, then spawns a
//!   background task that streams the file into the series directory and queues a
//!   scan job (same "download straight into the library" model as Telegram).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sqlx::Row;
use tokio::io::AsyncWriteExt;
use tracing::info;
use uuid::Uuid;

use parsers::{extract_series_name_from_filename, extract_volume, title_matches_series};

use crate::downloads::target::{finalize_download, prepare_destination, DownloadProgress};
use crate::downloads::worker::DownloadQueue;
use crate::error::ApiError;
use crate::state::AppState;

use super::types::{
    DdlProviderDownloadDto, DdlProviderSearchResponse, DdlProviderSearchResultDto,
    StartDdlProviderDownloadRequest,
};
use super::{load_config, require_enabled, DdlProviderFile, BROWSER_USER_AGENT, DOWNLOAD_TIMEOUT};

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

pub async fn search(
    state: &AppState,
    provider_id: &str,
    req: super::types::DdlProviderSearchRequest,
) -> Result<DdlProviderSearchResponse, ApiError> {
    let config = load_config(&state.pool).await?;
    let (provider, base_url) = require_enabled(&config, provider_id)?;
    let meta = provider.meta();

    let query = req
        .query
        .as_deref()
        .filter(|q| !q.trim().is_empty())
        .or_else(|| req.series_name.as_deref().filter(|q| !q.trim().is_empty()))
        .ok_or_else(|| ApiError::bad_request("query or series_name is required"))?
        .trim()
        .to_string();

    let limit = req.limit.unwrap_or(50).clamp(1, 100);
    let files = provider.search(&base_url, &query, limit).await?;

    let library_id = req
        .library_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let owned = match (library_id, req.series_name.as_deref()) {
        (Some(lib), Some(name)) if !name.trim().is_empty() => {
            owned_volumes(&state.pool, lib, name).await?
        }
        _ => HashSet::new(),
    };

    let mut results: Vec<DdlProviderSearchResultDto> = files
        .into_iter()
        .map(|file| {
            build_result(
                meta.id,
                &file,
                req.series_name.as_deref(),
                &req.missing_volumes,
                &owned,
            )
        })
        .collect();

    attach_downloads(&state.pool, meta.id, &mut results).await?;
    rank(&mut results);

    Ok(DdlProviderSearchResponse {
        provider: meta.id.to_string(),
        query,
        results,
    })
}

pub(super) fn build_result(
    provider_id: &str,
    file: &DdlProviderFile,
    requested_series: Option<&str>,
    missing: &[i32],
    owned: &HashSet<i32>,
) -> DdlProviderSearchResultDto {
    // DDL providers may expose a name `parsers` cannot read directly (fourtoutici
    // writes a bare `<series> <tome>`), so matching runs on `match_name`.
    let match_name = file.match_name.as_deref().unwrap_or(&file.filename);

    let series_name = requested_series
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let extracted = extract_series_name_from_filename(match_name);
            (!extracted.is_empty()).then_some(extracted)
        });

    let series_ref = series_name.as_deref().unwrap_or("");
    // A file whose name does not contain the requested series must not be
    // credited with matching missing volumes (e.g. another series' volume 5).
    let series_matches = series_ref.is_empty() || title_matches_series(match_name, series_ref);

    let (mut matched_missing_volumes, all_volumes) =
        parsers::match_title_volumes(match_name, missing);
    if !series_matches {
        matched_missing_volumes.clear();
    }

    let confidence = if series_matches && !missing.is_empty() {
        parsers::match_release_title(match_name, series_ref, missing)
            .map(|m| m.confidence.as_str().to_string())
    } else {
        None
    };

    let volume_number = matched_missing_volumes
        .first()
        .copied()
        .or_else(|| all_volumes.first().copied())
        .or_else(|| extract_volume(match_name));

    let already_owned = volume_number.is_some_and(|v| owned.contains(&v));

    DdlProviderSearchResultDto {
        provider: provider_id.to_string(),
        external_id: file.external_id.clone(),
        filename: file.filename.clone(),
        size: file.size,
        series_name,
        volume_number,
        all_volumes,
        matched_missing_volumes,
        match_confidence: confidence,
        already_owned,
        download_id: None,
        download_status: None,
    }
}

/// Regular volumes already present in the library for a series.
pub(super) async fn owned_volumes(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
) -> Result<HashSet<i32>, ApiError> {
    let rows = sqlx::query(
        "SELECT DISTINCT b.volume FROM books b \
         JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 AND norm_text(s.name) = norm_text($2) \
           AND b.volume_type = 'regular' AND b.volume IS NOT NULL",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .filter_map(|r| r.get::<Option<i32>, _>("volume"))
        .collect())
}

/// Flag results that already have a `ddl_provider_downloads` row.
async fn attach_downloads(
    pool: &sqlx::PgPool,
    provider_id: &str,
    results: &mut [DdlProviderSearchResultDto],
) -> Result<(), ApiError> {
    if results.is_empty() {
        return Ok(());
    }

    let ids: Vec<String> = results.iter().map(|r| r.external_id.clone()).collect();
    let rows = sqlx::query(
        "SELECT id, external_id, status FROM ddl_provider_downloads \
         WHERE provider = $1 AND external_id = ANY($2)",
    )
    .bind(provider_id)
    .bind(&ids)
    .fetch_all(pool)
    .await?;

    let mut existing: HashMap<String, (String, String)> = HashMap::new();
    for row in rows {
        existing.insert(
            row.get("external_id"),
            (row.get::<Uuid, _>("id").to_string(), row.get("status")),
        );
    }

    for result in results.iter_mut() {
        if let Some((id, status)) = existing.get(&result.external_id) {
            result.download_id = Some(id.clone());
            result.download_status = Some(status.clone());
        }
    }
    Ok(())
}

/// Missing-volume matches first, then files not yet owned, then by volume/name.
pub(super) fn rank(results: &mut [DdlProviderSearchResultDto]) {
    results.sort_by(|a, b| {
        b.matched_missing_volumes
            .len()
            .cmp(&a.matched_missing_volumes.len())
            .then(a.already_owned.cmp(&b.already_owned))
            .then(
                a.volume_number
                    .unwrap_or(i32::MAX)
                    .cmp(&b.volume_number.unwrap_or(i32::MAX)),
            )
            .then(a.filename.cmp(&b.filename))
    });
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

pub async fn start_ddl_download(
    state: &AppState,
    provider_id: &str,
    req: StartDdlProviderDownloadRequest,
) -> Result<Uuid, ApiError> {
    let config = load_config(&state.pool).await?;
    let (provider, base_url) = require_enabled(&config, provider_id)?;

    let library_id = Uuid::parse_str(&req.library_id)
        .map_err(|_| ApiError::bad_request("invalid library_id"))?;

    let library_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM libraries WHERE id = $1")
            .bind(library_id)
            .fetch_one(&state.pool)
            .await?;
    if library_exists == 0 {
        return Err(ApiError::not_found("library not found"));
    }
    if req.series_name.trim().is_empty() {
        return Err(ApiError::bad_request("series_name is required"));
    }

    let file_url = provider.file_url(&base_url, &req.external_id);

    // One row per provider+file, reused as the retry path (a failed or imported
    // file can be re-queued). A re-click while the transfer is already queued or
    // running must NOT start a second one: that would clobber the shared `.tmp`
    // file and overwrite the abort handle, leaving the first transfer
    // uncancellable.
    let download_id = match queue_ddl_download(&state.pool, provider_id, &req, library_id).await? {
        Some(id) => id,
        None => return Err(ApiError::conflict("this file is already being downloaded")),
    };

    let filename = req.filename.clone();
    let series_name = req.series_name.clone();
    let volume_number = req.volume_number;

    state
        .ddl_provider_downloads
        .spawn(
            state.pool.clone(),
            download_id,
            Arc::new(DdlDownloadQueue),
            move |pool| {
                Box::pin(async move {
                    do_download(
                        &pool,
                        download_id,
                        file_url,
                        filename,
                        series_name,
                        library_id,
                        volume_number,
                    )
                    .await
                })
            },
        )
        .await;

    Ok(download_id)
}

/// Insert a new `queued` row for a file, or re-queue an already-attempted one.
/// Returns `None` when a transfer for the same `(provider, external_id)` is
/// already queued or running, so the caller does not start a second one.
pub(super) async fn queue_ddl_download(
    pool: &sqlx::PgPool,
    provider_id: &str,
    req: &StartDdlProviderDownloadRequest,
    library_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO ddl_provider_downloads \
            (id, provider, external_id, filename, file_size, series_name, volume_number, library_id, status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'queued') \
         ON CONFLICT (provider, external_id) DO UPDATE SET \
            status = 'queued', bytes_downloaded = 0, error_message = NULL, \
            filename = EXCLUDED.filename, file_size = EXCLUDED.file_size, \
            series_name = EXCLUDED.series_name, volume_number = EXCLUDED.volume_number, \
            library_id = EXCLUDED.library_id, updated_at = NOW() \
         WHERE ddl_provider_downloads.status NOT IN ('queued', 'downloading') \
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(provider_id)
    .bind(&req.external_id)
    .bind(&req.filename)
    .bind(req.size)
    .bind(&req.series_name)
    .bind(req.volume_number)
    .bind(library_id)
    .fetch_optional(pool)
    .await
}

/// Claim/fail SQL for the DDL provider download queue.
struct DdlDownloadQueue;

#[async_trait::async_trait]
impl DownloadQueue for DdlDownloadQueue {
    async fn claim(&self, pool: &sqlx::PgPool, id: Uuid) -> bool {
        start_downloading(pool, id).await
    }

    async fn mark_failed(&self, pool: &sqlx::PgPool, id: Uuid, error: &str) {
        // Do not resurrect a download dismissed while it was failing.
        let _ = sqlx::query(
            "UPDATE ddl_provider_downloads SET status = 'failed', error_message = $2, updated_at = NOW() \
             WHERE id = $1 AND status <> 'dismissed'",
        )
        .bind(id)
        .bind(error)
        .execute(pool)
        .await;
    }
}

/// Flip a `queued` row to `downloading`. Returns `false` when the row is no
/// longer queued (dismissed or deleted).
async fn start_downloading(pool: &sqlx::PgPool, id: Uuid) -> bool {
    sqlx::query(
        "UPDATE ddl_provider_downloads SET status = 'downloading', error_message = NULL, updated_at = NOW() \
         WHERE id = $1 AND status = 'queued'",
    )
    .bind(id)
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

async fn do_download(
    pool: &sqlx::PgPool,
    download_id: Uuid,
    file_url: String,
    filename: String,
    series_name: String,
    library_id: Uuid,
    volume_number: Option<i32>,
) -> anyhow::Result<()> {
    let destination =
        prepare_destination(pool, library_id, &series_name, &filename, volume_number).await?;
    let dest_path = destination.dest_path;
    let tmp_path = destination.tmp_path;

    info!(
        "DDL provider download {download_id}: {filename} → {}",
        dest_path.display()
    );

    let client = stripstream_core::http::build_http_client_with_custom_agent(
        DOWNLOAD_TIMEOUT,
        BROWSER_USER_AGENT,
    )?;

    let result = tokio::time::timeout(DOWNLOAD_TIMEOUT, async {
        let mut response = client.get(&file_url).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("DDL provider returned HTTP {}", response.status());
        }

        let mut file = tokio::fs::File::create(&tmp_path).await?;
        let mut progress = DownloadProgress::new();

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
            if let Some(written) = progress.advance(chunk.len() as i64) {
                let _ = sqlx::query(
                    "UPDATE ddl_provider_downloads SET bytes_downloaded = $1, updated_at = NOW() WHERE id = $2",
                )
                .bind(written)
                .bind(download_id)
                .execute(pool)
                .await;
            }
        }

        file.flush().await?;
        Ok::<i64, anyhow::Error>(progress.written())
    })
    .await;

    let bytes_written = match result {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(e);
        }
        Err(_) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            anyhow::bail!("download timed out after 30 minutes");
        }
    };

    if bytes_written == 0 {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        anyhow::bail!("DDL provider returned an empty file");
    }

    // Move into place and queue a scan so the new file is indexed like any other
    // discovered book.
    let scan_job_id = finalize_download(pool, library_id, &tmp_path, &dest_path).await?;

    sqlx::query(
        "UPDATE ddl_provider_downloads \
         SET status = 'imported', bytes_downloaded = $2, file_size = COALESCE(file_size, $2), \
             volume_number = COALESCE(volume_number, $3), error_message = NULL, updated_at = NOW() \
         WHERE id = $1",
    )
    .bind(download_id)
    .bind(bytes_written)
    .bind(volume_number)
    .execute(pool)
    .await?;

    info!(
        "DDL provider download {download_id} complete → {} (scan job {scan_job_id})",
        dest_path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Listing / dismissal
// ---------------------------------------------------------------------------

pub async fn list_ddl_downloads(state: &AppState) -> Result<Vec<DdlProviderDownloadDto>, ApiError> {
    let rows = sqlx::query(
        "SELECT d.id, d.provider, d.external_id, d.filename, d.file_size, d.bytes_downloaded, \
                d.series_name, d.volume_number, d.status, d.error_message, d.created_at, d.updated_at, \
                d.library_id, l.name AS library_name, sr.id AS series_id \
         FROM ddl_provider_downloads d \
         LEFT JOIN libraries l ON l.id = d.library_id \
         LEFT JOIN series sr ON sr.library_id = d.library_id \
              AND norm_text(sr.name) = norm_text(d.series_name) \
         WHERE d.status <> 'dismissed' \
         ORDER BY d.created_at DESC \
         LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| DdlProviderDownloadDto {
            id: row.get::<Uuid, _>("id").to_string(),
            provider: row.get("provider"),
            external_id: row.get("external_id"),
            filename: row.get("filename"),
            file_size: row.get("file_size"),
            bytes_downloaded: row.get("bytes_downloaded"),
            series_name: row.get("series_name"),
            series_id: row
                .get::<Option<Uuid>, _>("series_id")
                .map(|v| v.to_string()),
            volume_number: row.get("volume_number"),
            library_id: row
                .get::<Option<Uuid>, _>("library_id")
                .map(|v| v.to_string()),
            library_name: row.get("library_name"),
            status: row.get("status"),
            error_message: row.get("error_message"),
            created_at: row
                .get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                .to_rfc3339(),
            updated_at: row
                .get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
                .to_rfc3339(),
        })
        .collect())
}

pub async fn dismiss_ddl_download(state: &AppState, id: Uuid, hard: bool) -> Result<(), ApiError> {
    state.ddl_provider_downloads.cancel(id).await;

    let affected = if hard {
        sqlx::query("DELETE FROM ddl_provider_downloads WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await?
            .rows_affected()
    } else {
        sqlx::query(
            "UPDATE ddl_provider_downloads SET status = 'dismissed', updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected()
    };

    if affected == 0 {
        return Err(ApiError::not_found("download not found"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Misc helpers
// ---------------------------------------------------------------------------

/// Probe a provider with a minimal query (settings "test" button).
pub async fn probe(state: &AppState, provider_id: &str, query: &str) -> Result<usize, ApiError> {
    let config = load_config(&state.pool).await?;
    let (provider, base_url) = require_enabled(&config, provider_id)?;
    let files = provider.search(&base_url, query, 1).await?;
    Ok(files.len())
}

#[cfg(test)]
mod tests {
    use super::super::{fourtoutici::normalize_for_match, save_config, DdlProvidersConfig};
    use super::*;

    /// Build a DDL provider file the way `fourtoutici::parse_files` does, i.e. with
    /// the rewritten `match_name`.
    fn file(original: &str) -> DdlProviderFile {
        DdlProviderFile {
            external_id: "id".to_string(),
            filename: original.to_string(),
            match_name: normalize_for_match(original),
            size: None,
        }
    }

    #[test]
    fn flags_missing_volume_match_for_requested_series() {
        let result = build_result(
            "fourtoutici",
            &file("BD Astérix le Gaulois 03 Astérix et les Goths – René Goscinny.pdf"),
            Some("Astérix le Gaulois"),
            &[3, 4],
            &HashSet::new(),
        );

        assert_eq!(result.matched_missing_volumes, vec![3]);
        assert_eq!(result.volume_number, Some(3));
        assert_eq!(result.match_confidence.as_deref(), Some("high"));
        assert!(!result.already_owned);
    }

    #[test]
    fn does_not_credit_missing_volumes_across_series() {
        let result = build_result(
            "fourtoutici",
            &file("BD Tintin 03 Le Crabe aux pinces d'or – Hergé.pdf"),
            Some("Astérix le Gaulois"),
            &[3],
            &HashSet::new(),
        );

        assert!(result.matched_missing_volumes.is_empty());
        assert_eq!(result.volume_number, Some(3));
        assert!(result.match_confidence.is_none());
    }

    #[test]
    fn marks_already_owned_volumes() {
        let owned: HashSet<i32> = [3].into_iter().collect();
        let result = build_result(
            "fourtoutici",
            &file("BD Astérix le Gaulois 03 Astérix et les Goths – René Goscinny.pdf"),
            Some("Astérix le Gaulois"),
            &[3],
            &owned,
        );
        assert!(result.already_owned);
    }

    #[test]
    fn infers_series_from_filename_when_absent() {
        let result = build_result(
            "fourtoutici",
            &file("EBOOK Gon - Tome 5 La peur – Michael Grant.epub"),
            None,
            &[],
            &HashSet::new(),
        );

        assert_eq!(result.series_name.as_deref(), Some("Gon"));
        assert_eq!(result.volume_number, Some(5));
    }

    #[test]
    fn falls_back_to_raw_filename_without_match_name() {
        // Already in a shape `parsers` understands → no rewrite needed.
        let plain = DdlProviderFile {
            external_id: "id".to_string(),
            filename: "Astérix le Gaulois T03.cbz".to_string(),
            match_name: None,
            size: None,
        };
        let result = build_result(
            "fourtoutici",
            &plain,
            Some("Astérix le Gaulois"),
            &[3],
            &HashSet::new(),
        );

        assert_eq!(result.volume_number, Some(3));
        assert_eq!(result.matched_missing_volumes, vec![3]);
    }

    #[test]
    fn ranking_puts_missing_matches_first() {
        let mut results = vec![
            build_result(
                "p",
                &file("BD A 01 Premier – X.pdf"),
                Some("A"),
                &[2],
                &HashSet::new(),
            ),
            build_result(
                "p",
                &file("BD A 02 Second – X.pdf"),
                Some("A"),
                &[2],
                &HashSet::new(),
            ),
            build_result(
                "p",
                &file("BD A 03 Troisieme – X.pdf"),
                Some("A"),
                &[2],
                &HashSet::new(),
            ),
        ];
        rank(&mut results);
        assert_eq!(results[0].volume_number, Some(2));
    }

    #[test]
    fn require_enabled_guards_unknown_and_disabled_providers() {
        let unknown = require_enabled(&DdlProvidersConfig::default(), "does-not-exist");
        assert!(unknown.is_err());

        // An empty config enables nothing, even though the migration seeds fourtoutici.
        let disabled = require_enabled(&DdlProvidersConfig::default(), "fourtoutici");
        assert!(disabled.is_err());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn config_round_trips_through_app_settings(pool: sqlx::PgPool) {
        let config = load_config(&pool).await.expect("seeded config loads");
        assert!(
            !config.providers["fourtoutici"].enabled,
            "migration 0120 seeds fourtoutici disabled (opt-in)"
        );

        let mut updated = config;
        updated.providers.get_mut("fourtoutici").unwrap().enabled = true;
        save_config(&pool, &updated).await.unwrap();

        let reloaded = load_config(&pool).await.unwrap();
        assert!(reloaded.providers["fourtoutici"].enabled);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn attach_downloads_flags_rows_by_external_id(pool: sqlx::PgPool) {
        let download_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ddl_provider_downloads \
                (id, provider, external_id, filename, file_size, series_name, volume_number, status) \
             VALUES ($1, 'fourtoutici', 'ext-1', 'BD A 01 Premier.pdf', 1024, 'A', 1, 'imported')",
        )
        .bind(download_id)
        .execute(&pool)
        .await
        .unwrap();

        let mut results = vec![
            build_result(
                "fourtoutici",
                &file("BD A 01 Premier – X.pdf"),
                Some("A"),
                &[],
                &HashSet::new(),
            ),
            build_result(
                "fourtoutici",
                &file("BD A 02 Second – X.pdf"),
                Some("A"),
                &[],
                &HashSet::new(),
            ),
        ];
        results[0].external_id = "ext-1".to_string();
        results[1].external_id = "ext-2".to_string();

        attach_downloads(&pool, "fourtoutici", &mut results)
            .await
            .unwrap();

        assert_eq!(
            results[0].download_id.as_deref(),
            Some(download_id.to_string().as_str())
        );
        assert_eq!(results[0].download_status.as_deref(), Some("imported"));
        assert!(results[1].download_id.is_none());
    }

    fn start_request() -> StartDdlProviderDownloadRequest {
        StartDdlProviderDownloadRequest {
            external_id: "ext-1".to_string(),
            filename: "BD A 01 Premier.pdf".to_string(),
            size: Some(1024),
            library_id: Uuid::new_v4().to_string(),
            series_name: "A".to_string(),
            volume_number: Some(1),
        }
    }

    async fn seed_library(pool: &sqlx::PgPool) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("test-lib")
            .bind("/libraries/test-lib")
            .execute(pool)
            .await
            .unwrap();
        library_id
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn queue_ddl_download_refuses_a_file_already_in_flight(pool: sqlx::PgPool) {
        let library_id = seed_library(&pool).await;
        let req = start_request();

        let download_id = queue_ddl_download(&pool, "fourtoutici", &req, library_id)
            .await
            .unwrap()
            .expect("first click queues the file");

        // A second click while the transfer is queued must not spawn a twin.
        assert!(queue_ddl_download(&pool, "fourtoutici", &req, library_id)
            .await
            .unwrap()
            .is_none());

        // Nor once it is running.
        sqlx::query("UPDATE ddl_provider_downloads SET status = 'downloading' WHERE id = $1")
            .bind(download_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(queue_ddl_download(&pool, "fourtoutici", &req, library_id)
            .await
            .unwrap()
            .is_none());

        // A finished/attempted row can be retried, reusing the same row.
        sqlx::query("UPDATE ddl_provider_downloads SET status = 'failed' WHERE id = $1")
            .bind(download_id)
            .execute(&pool)
            .await
            .unwrap();
        let retried = queue_ddl_download(&pool, "fourtoutici", &req, library_id)
            .await
            .unwrap()
            .expect("a failed row can be re-queued");
        assert_eq!(retried, download_id, "retry reuses the same row");

        let status: String =
            sqlx::query_scalar("SELECT status FROM ddl_provider_downloads WHERE id = $1")
                .bind(download_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "queued");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn mark_failed_keeps_dismissed_rows_dismissed(pool: sqlx::PgPool) {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ddl_provider_downloads \
                (id, provider, external_id, filename, series_name, status) \
             VALUES ($1, 'fourtoutici', 'ext-1', 'a.pdf', 'A', 'dismissed')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

        DdlDownloadQueue.mark_failed(&pool, id, "boom").await;

        let status: String =
            sqlx::query_scalar("SELECT status FROM ddl_provider_downloads WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            status, "dismissed",
            "a download dismissed while failing must not reappear as failed"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn lists_and_dismisses_downloads(pool: sqlx::PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("test-lib")
            .bind("/libraries/test-lib")
            .execute(&pool)
            .await
            .unwrap();

        let download_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ddl_provider_downloads \
                (id, provider, external_id, filename, file_size, series_name, volume_number, library_id, status) \
             VALUES ($1, 'fourtoutici', 'ext-1', 'BD A 03 Troisieme.pdf', 1024, 'A', 3, $2, 'imported')",
        )
        .bind(download_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let state = crate::state::test_state(pool);
        let downloads = list_ddl_downloads(&state).await.unwrap();
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].volume_number, Some(3));
        assert_eq!(downloads[0].library_name.as_deref(), Some("test-lib"));
        assert_eq!(downloads[0].status, "imported");

        dismiss_ddl_download(&state, download_id, false)
            .await
            .unwrap();
        assert!(
            list_ddl_downloads(&state).await.unwrap().is_empty(),
            "soft-dismissed rows are hidden from the list"
        );

        dismiss_ddl_download(&state, download_id, true)
            .await
            .unwrap();
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ddl_provider_downloads WHERE id = $1")
                .bind(download_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0, "hard dismiss deletes the row");

        assert!(dismiss_ddl_download(&state, Uuid::new_v4(), true)
            .await
            .is_err());
    }
}
