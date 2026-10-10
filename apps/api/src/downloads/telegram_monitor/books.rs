use axum::extract::{Path as AxumPath, Query};
use axum::{extract::State, Json};
use chrono::Utc;
use serde_json::Value;
use sqlx::Row;
use tracing::{error, info, warn};
use uuid::Uuid;

use parsers::{detect_format, parse_metadata_fast};

use crate::books::rename::{
    load_rename_max_volume, load_rename_templates, render_rename_filename, RenameTemplateBook,
};
use crate::downloads::{import_pipeline::find_existing_series_dir, missing};
use crate::{error::ApiError, state::AppState};
use stripstream_core::paths::remap_libraries_path;

use super::auth::{load_tg_settings, save_session_to_db};
use super::channels::insert_document_message;
use super::types::*;

/// Find the physical target directory for a series in a library,
/// matching existing book files first, then existing directories, then fallback to new.
pub async fn resolve_target_dir(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
) -> anyhow::Result<String> {
    // 1. Try DB: find existing book files for this series
    let any_row = sqlx::query(
        "SELECT bf.abs_path FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND norm_text(s.name) = norm_text($2) \
         LIMIT 1",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    if let Some(r) = any_row {
        let abs_path: String = r.get("abs_path");
        let physical = remap_libraries_path(&abs_path);
        let parent = std::path::Path::new(&physical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(physical);
        info!("[TG] Series '{}' DB match → {}", series_name, parent);
        return Ok(parent);
    }

    // 2. No DB match: look for existing directory in library root
    let lib_row = sqlx::query("SELECT root_path FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(pool)
        .await?;
    let root_path: String = lib_row.get("root_path");
    let physical_root = remap_libraries_path(&root_path);

    let dir = find_existing_series_dir(&physical_root, series_name)
        .unwrap_or_else(|| format!("{}/{}", physical_root.trim_end_matches('/'), series_name));

    info!("[TG] Series '{}' dir → {}", series_name, dir);
    Ok(dir)
}

pub async fn refresh_reimportable_telegram_links(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    let result = sqlx::query(
        "UPDATE telegram_book_links b
         SET status = 'available',
             bytes_downloaded = 0,
             error_message = NULL,
             updated_at = NOW()
         FROM telegram_sources src
         WHERE src.id = b.source_id
           AND b.status = 'imported'
           AND b.updated_at < NOW() - INTERVAL '2 minutes'
           AND b.series_name IS NOT NULL
           AND NOT EXISTS (
             SELECT 1
             FROM series s
             JOIN books bk ON bk.series_id = s.id
             WHERE s.library_id = COALESCE(b.library_id, src.library_id)
               AND norm_text(s.name) = norm_text(b.series_name)
               AND (
                 b.volume_number IS NULL
                 OR bk.volume = b.volume_number
               )
           )",
    )
    .execute(pool)
    .await?;

    let count = result.rows_affected();
    if count > 0 {
        info!("[TG] Marked {count} stale imported link(s) as available again");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/books
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/books",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramBookLinkDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_books(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramBookLinkDto>>, ApiError> {
    refresh_reimportable_telegram_links(&state.pool).await?;

    let rows = sqlx::query(
        "SELECT b.id, b.source_id, s.channel_username, b.message_id, b.filename, \
                b.file_size, b.mime_type, b.message_text, b.status, b.library_id, \
                b.book_id, b.error_message, b.series_name, b.volume_number, b.created_at \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.status != 'dismissed' \
         ORDER BY b.created_at DESC \
         LIMIT 500",
    )
    .fetch_all(&state.pool)
    .await?;

    let books = rows
        .iter()
        .map(|r| TelegramBookLinkDto {
            id: r.get::<Uuid, _>("id").to_string(),
            source_id: r.get::<Uuid, _>("source_id").to_string(),
            channel_username: r.get("channel_username"),
            message_id: r.get("message_id"),
            filename: r.get("filename"),
            file_size: r.get("file_size"),
            mime_type: r.get("mime_type"),
            message_text: r.get("message_text"),
            status: r.get("status"),
            library_id: r
                .get::<Option<Uuid>, _>("library_id")
                .map(|u| u.to_string()),
            book_id: r.get::<Option<Uuid>, _>("book_id").map(|u| u.to_string()),
            error_message: r.get("error_message"),
            series_name: r.get("series_name"),
            volume_number: r.get("volume_number"),
            created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        })
        .collect();

    Ok(Json(books))
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/available  — books grouped by series (for Downloads page)
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/available",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramAvailableGroupDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_available_by_series(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramAvailableGroupDto>>, ApiError> {
    refresh_reimportable_telegram_links(&state.pool).await?;

    // ROW_NUMBER CTE deduplicates per (source, series, volume): for non-null volumes keeps
    // most recent only; null-volume books each get their own partition via id so all survive.
    let rows = sqlx::query(
        "WITH ranked AS ( \
           SELECT b.id, b.source_id, b.message_id, b.filename, b.file_size, b.status, \
                  b.series_name, b.volume_number, b.created_at, b.library_id, \
                  ROW_NUMBER() OVER ( \
                    PARTITION BY b.source_id, b.series_name, \
                                 COALESCE(b.volume_number::text, b.id::text) \
                    ORDER BY b.created_at DESC \
                  ) AS rn \
           FROM telegram_book_links b \
           WHERE b.status IN ('available', 'failed') AND b.series_name IS NOT NULL \
         ) \
         SELECT r.id, r.source_id, s.channel_username, r.message_id, r.filename, \
                r.file_size, r.status, r.series_name, r.volume_number, r.created_at, \
                COALESCE(r.library_id, s.library_id) AS resolved_library_id \
         FROM ranked r \
         JOIN telegram_sources s ON s.id = r.source_id \
         WHERE r.rn = 1 \
         ORDER BY r.series_name, r.volume_number NULLS LAST, r.created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    // Collect unique library IDs and (series_name, library_id) pairs in one pass
    let mut lib_id_set: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    let mut series_key_set: std::collections::HashSet<(String, Uuid)> =
        std::collections::HashSet::new();
    for r in &rows {
        if let Some(lid) = r.get::<Option<Uuid>, _>("resolved_library_id") {
            lib_id_set.insert(lid);
            if let Some(sn) = r.get::<Option<String>, _>("series_name") {
                series_key_set.insert((sn, lid));
            }
        }
    }
    let lib_ids: Vec<Uuid> = lib_id_set.into_iter().collect();
    let series_keys: Vec<(String, Uuid)> = series_key_set.into_iter().collect();

    // Fetch library names in one query
    let mut lib_names: std::collections::HashMap<Uuid, String> = std::collections::HashMap::new();
    if !lib_ids.is_empty() {
        let libs = sqlx::query("SELECT id, name FROM libraries WHERE id = ANY($1)")
            .bind(&lib_ids)
            .fetch_all(&state.pool)
            .await?;
        for r in &libs {
            lib_names.insert(r.get("id"), r.get("name"));
        }
    }

    // Batch-resolve all (series_name, library_id) → series_id in a single UNNEST query
    let mut series_ids: std::collections::HashMap<(String, Uuid), Option<Uuid>> =
        std::collections::HashMap::new();
    if !series_keys.is_empty() {
        let (names, libs): (Vec<String>, Vec<Uuid>) = series_keys
            .iter()
            .map(|(sn, lid)| (sn.clone(), *lid))
            .unzip();
        let batch = sqlx::query(
            "SELECT q.sn AS series_name, q.lid AS library_id, s.id AS series_id \
             FROM UNNEST($1::text[], $2::uuid[]) AS q(sn, lid) \
             LEFT JOIN series s ON s.library_id = q.lid \
               AND norm_text(s.name) = norm_text(q.sn)",
        )
        .bind(&names)
        .bind(&libs)
        .fetch_all(&state.pool)
        .await?;
        for r in &batch {
            let sn: String = r.get("series_name");
            let lid: Uuid = r.get("library_id");
            let sid: Option<Uuid> = r.get("series_id");
            series_ids.entry((sn, lid)).or_insert(sid);
        }
        // Ensure every key is present (unmatched series get None)
        for (sn, lid) in &series_keys {
            series_ids.entry((sn.clone(), *lid)).or_insert(None);
        }
    }

    // Batch-fetch owned volumes and series total_volumes for computing missing count
    let matched_series_ids: Vec<Uuid> = series_ids.values().filter_map(|o| *o).collect();
    let availability_by_series =
        missing::load_series_availability(&state.pool, &matched_series_ids).await?;

    // Group books preserving insertion order (SQL ORDER BY series_name)
    let mut group_keys: Vec<(String, Uuid)> = Vec::new();
    let mut groups: std::collections::HashMap<(String, Uuid), Vec<TelegramAvailableBookDto>> =
        std::collections::HashMap::new();
    for r in &rows {
        let series_name: Option<String> = r.get("series_name");
        let lib_id: Option<Uuid> = r.get("resolved_library_id");
        if let (Some(sn), Some(lid)) = (series_name, lib_id) {
            let key = (sn.clone(), lid);
            if !groups.contains_key(&key) {
                group_keys.push(key.clone());
            }
            let book = TelegramAvailableBookDto {
                id: r.get::<Uuid, _>("id").to_string(),
                channel_username: r.get("channel_username"),
                message_id: r.get("message_id"),
                filename: r.get("filename"),
                file_size: r.get("file_size"),
                volume_number: r.get("volume_number"),
                status: r.get("status"),
                created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
            };
            groups.entry(key).or_default().push(book);
        }
    }

    // Only return groups where the series name was matched to a local series
    let result: Vec<TelegramAvailableGroupDto> = group_keys
        .into_iter()
        .filter_map(|(series_name, lib_id)| {
            let sid_uuid = series_ids
                .get(&(series_name.clone(), lib_id))
                .and_then(|o| *o);
            let series_id = sid_uuid.map(|u| u.to_string());
            series_id.as_ref()?; // skip unmatched groups
            let owned_volumes = sid_uuid
                .and_then(|sid| availability_by_series.get(&sid))
                .map(|availability| availability.owned_volumes.clone())
                .unwrap_or_default();
            let series_missing_count = sid_uuid
                .and_then(|sid| availability_by_series.get(&sid))
                .map(|availability| availability.missing_count)
                .unwrap_or(0);
            let has_integral = sid_uuid
                .and_then(|sid| availability_by_series.get(&sid))
                .map(|availability| availability.has_integral)
                .unwrap_or(false);
            if has_integral {
                return None;
            }
            let books = groups.remove(&(series_name.clone(), lib_id))?;
            Some(TelegramAvailableGroupDto {
                series_id,
                library_name: lib_names.get(&lib_id).cloned().unwrap_or_default(),
                library_id: lib_id.to_string(),
                owned_volumes,
                series_missing_count,
                series_name,
                books,
            })
        })
        .collect();

    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/downloads  — in-progress / imported / failed books
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/downloads",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramDownloadItemDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_downloads(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramDownloadItemDto>>, ApiError> {
    refresh_reimportable_telegram_links(&state.pool).await?;

    let rows = sqlx::query(
        "SELECT b.id, b.series_name, src.channel_username, b.filename, b.file_size,
                b.bytes_downloaded, b.volume_number, b.status, b.error_message, b.created_at, b.updated_at,
                COALESCE(b.library_id, src.library_id) AS resolved_library_id,
                l.name AS library_name,
                sr.id AS series_id
         FROM telegram_book_links b
         JOIN telegram_sources src ON src.id = b.source_id
         LEFT JOIN libraries l ON l.id = COALESCE(b.library_id, src.library_id)
         LEFT JOIN series sr ON norm_text(sr.name) = norm_text(b.series_name)
           AND sr.library_id = COALESCE(b.library_id, src.library_id)
         WHERE b.status IN ('queued', 'downloading', 'imported', 'failed')
         ORDER BY b.created_at DESC
         LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .iter()
        .map(|r| TelegramDownloadItemDto {
            id: r.get::<Uuid, _>("id").to_string(),
            series_name: r.get("series_name"),
            series_id: r.get::<Option<Uuid>, _>("series_id").map(|u| u.to_string()),
            library_id: r
                .get::<Option<Uuid>, _>("resolved_library_id")
                .map(|u| u.to_string()),
            library_name: r.get("library_name"),
            channel_username: r.get("channel_username"),
            filename: r.get("filename"),
            file_size: r.get("file_size"),
            bytes_downloaded: r.get::<i64, _>("bytes_downloaded"),
            volume_number: r.get("volume_number"),
            status: r.get("status"),
            error_message: r.get("error_message"),
            created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
            updated_at: r.get::<chrono::DateTime<Utc>, _>("updated_at").to_rfc3339(),
        })
        .collect();

    Ok(Json(items))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/books/:id/download
// ---------------------------------------------------------------------------

#[utoipa::path(
    post, path = "/telegram-monitor/books/{id}/download",
    tag = "telegram-monitor",
    params(("id" = String, Path, description = "Book link UUID")),
    request_body = DownloadBookRequest,
    responses(
        (status = 200, description = "Download started"),
        (status = 400, description = "Not configured or not authorized"),
        (status = 404, description = "Book link not found"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn download_book(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
    Json(body): Json<DownloadBookRequest>,
) -> Result<Json<Value>, ApiError> {
    let (api_id, api_hash, _, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Telegram monitor not configured"))?;

    let session_bytes = session_bytes
        .ok_or_else(|| ApiError::bad_request("Not authorized — complete auth first"))?;

    // Resolve library_id: from request, then from the link itself, then from the source
    let override_lib = body
        .library_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());

    let row = sqlx::query(
        "SELECT b.source_id, b.message_id, b.filename, b.status, b.series_name, \
                COALESCE($2::uuid, b.library_id, s.library_id) AS library_id, \
                s.channel_username \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.id = $1",
    )
    .bind(id)
    .bind(override_lib)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("book link not found"))?;

    let status: String = row.get("status");
    if status == "queued" || status == "downloading" {
        return Err(ApiError::bad_request(format!("already {status}")));
    }

    let source_id: Uuid = row.get("source_id");
    let message_id: i64 = row.get("message_id");
    let filename: String = row.get("filename");
    let library_id: Option<Uuid> = row.get("library_id");
    let channel_username: String = row.get("channel_username");
    let series_name: Option<String> = row.get("series_name");

    let library_id =
        library_id.ok_or_else(|| ApiError::bad_request("No library configured for this source"))?;

    // series_name fallback: extract from filename if not stored
    let series_name =
        series_name.unwrap_or_else(|| parsers::extract_series_name_from_filename(&filename));

    // Mark as queued (will switch to 'downloading' once the semaphore permit is acquired)
    sqlx::query(
        "UPDATE telegram_book_links SET status = 'queued', bytes_downloaded = 0, updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    let pool = state.pool.clone();
    let api_id_i32 = api_id as i32;
    let download_limit = state.telegram_download_limit.clone();
    let abort_handles = state.telegram_abort_handles.clone();

    let handle = tokio::spawn(async move {
        // Acquire semaphore permit — waits here if the concurrent download limit is reached
        let _permit = download_limit.acquire_owned().await;

        // Check if dismissed while waiting for the permit
        let current =
            sqlx::query_scalar::<_, String>("SELECT status FROM telegram_book_links WHERE id = $1")
                .bind(id)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
        if current == "dismissed" {
            abort_handles.lock().await.remove(&id);
            return;
        }

        // Switch to 'downloading' now that we have a slot
        let _ = sqlx::query(
            "UPDATE telegram_book_links SET status = 'downloading', updated_at = NOW() WHERE id = $1 AND status = 'queued'",
        )
        .bind(id)
        .execute(&pool)
        .await;

        match do_download(
            pool.clone(),
            api_id_i32,
            api_hash,
            session_bytes,
            id,
            source_id,
            message_id,
            channel_username,
            filename,
            series_name,
            library_id,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("Telegram download failed for {id}: {e}");
                // Don't overwrite 'dismissed' status
                let _ = sqlx::query(
                    "UPDATE telegram_book_links SET status = 'failed', error_message = $1, updated_at = NOW() WHERE id = $2 AND status != 'dismissed'",
                )
                .bind(e.to_string())
                .bind(id)
                .execute(&pool)
                .await;
            }
        }

        abort_handles.lock().await.remove(&id);
    });
    state
        .telegram_abort_handles
        .lock()
        .await
        .insert(id, handle.abort_handle());

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[allow(clippy::too_many_arguments)]
pub async fn do_download(
    pool: sqlx::PgPool,
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    link_id: Uuid,
    _source_id: Uuid,
    message_id: i64,
    channel_username: String,
    filename: String,
    series_name: String,
    library_id: Uuid,
) -> anyhow::Result<()> {
    use grammers_client::types::Downloadable;
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    let session = Session::load(&session_bytes)?;
    let client = Client::connect(Config {
        session,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await?;

    let chat = client
        .resolve_username(&channel_username)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Channel not found: {channel_username}"))?;

    // Find the specific message
    let mut iter = client
        .iter_messages(&chat)
        .offset_id((message_id + 1) as i32)
        .limit(1);
    let message = iter
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Message {message_id} not found"))?;

    if message.id() as i64 != message_id {
        anyhow::bail!(
            "Message ID mismatch: expected {message_id}, got {}",
            message.id()
        );
    }

    let media = message
        .media()
        .ok_or_else(|| anyhow::anyhow!("No media in message {message_id}"))?;

    // Resolve target directory using series matching
    let target_dir = resolve_target_dir(&pool, library_id, &series_name).await?;
    std::fs::create_dir_all(&target_dir)?;

    let safe_name = build_telegram_target_filename(&pool, library_id, &series_name, &filename)
        .await?
        .unwrap_or_else(|| sanitize_filename(&filename));
    let dest_path = std::path::Path::new(&target_dir).join(&safe_name);

    if dest_path.exists() {
        tokio::fs::remove_file(&dest_path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to remove existing file for re-import: {e}"))?;
    }

    let tmp_path = format!("{}.tmp", dest_path.display());

    info!("Downloading {} → {}", filename, dest_path.display());
    let downloadable = Downloadable::Media(media);

    // Stream download with progress tracking and FLOOD_WAIT retry
    const MAX_RETRIES: u32 = 3;
    const PROGRESS_UPDATE_BYTES: i64 = 1_048_576; // update DB every 1 MB
    const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);
    let mut retries = 0u32;
    loop {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        let result = tokio::time::timeout(DOWNLOAD_TIMEOUT, async {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::File::create(&tmp_path).await?;
            let mut iter = client.iter_download(&downloadable);
            let mut bytes_written: i64 = 0;
            let mut last_db_update: i64 = 0;
            while let Some(chunk) = iter.next().await.map_err(std::io::Error::other)? {
                file.write_all(&chunk).await?;
                bytes_written += chunk.len() as i64;
                if bytes_written - last_db_update >= PROGRESS_UPDATE_BYTES {
                    last_db_update = bytes_written;
                    let _ = sqlx::query(
                        "UPDATE telegram_book_links SET bytes_downloaded = $1, updated_at = NOW() WHERE id = $2",
                    )
                    .bind(bytes_written)
                    .bind(link_id)
                    .execute(&pool)
                    .await;
                }
            }
            file.flush().await?;
            Ok::<(), std::io::Error>(())
        }).await;
        match result {
            Ok(Ok(())) => break,
            Ok(Err(e)) => {
                let msg = e.to_string();
                if msg.contains("FLOOD_WAIT") && retries < MAX_RETRIES {
                    let wait_secs = parse_flood_wait_secs(&msg).unwrap_or(5);
                    warn!(
                        "FLOOD_WAIT {wait_secs}s for {filename}, retry {}/{MAX_RETRIES}",
                        retries + 1
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs + 1)).await;
                    retries += 1;
                } else {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(anyhow::anyhow!("{e}"));
                }
            }
            Err(_) => {
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return Err(anyhow::anyhow!(
                    "Download timeout after 30 minutes: {filename}"
                ));
            }
        }
    }

    tokio::fs::rename(&tmp_path, &dest_path).await?;

    let updated_bytes = client.session().save();
    save_session_to_db(&pool, updated_bytes)
        .await
        .map_err(|e| anyhow::anyhow!("session save: {:?}", e))?;

    info!("Downloaded {filename} to {}", dest_path.display());

    // Enqueue scan job
    let scan_job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(scan_job_id)
    .bind(library_id)
    .execute(&pool)
    .await?;

    // Mark imported
    sqlx::query(
        "UPDATE telegram_book_links SET status = 'imported', error_message = NULL, updated_at = NOW() WHERE id = $1",
    )
    .bind(link_id)
    .execute(&pool)
    .await?;

    info!("Telegram download complete: {filename}, scan job {scan_job_id} queued");
    Ok(())
}

pub fn parse_flood_wait_secs(err: &str) -> Option<u64> {
    // Error format: "rpc error 420: FLOOD_WAIT ... (value: 2)"
    err.split("value:")
        .nth(1)
        .and_then(|s| s.trim().split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse().ok())
}

pub fn sanitize_filename(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    if safe.is_empty() {
        "unknown".to_string()
    } else {
        safe
    }
}

pub async fn build_telegram_target_filename(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
    filename: &str,
) -> anyhow::Result<Option<String>> {
    let path = std::path::Path::new(filename);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(format) = detect_format(path) else {
        return Ok(None);
    };

    let parsed = parse_metadata_fast(path, format, std::path::Path::new(""));
    let fallback_volume = parsed.volume.unwrap_or(0);
    let max_volume = load_rename_max_volume(pool, library_id, series_name, fallback_volume).await?;
    let templates = load_rename_templates(pool).await?;

    let book = RenameTemplateBook {
        title: parsed.title,
        authors: Vec::new(),
        volume: parsed.volume,
        volume_type: parsed.volume_type.as_str().to_string(),
        publish_date: None,
        isbn: None,
        abs_path: filename.to_string(),
    };

    Ok(render_rename_filename(
        &templates,
        series_name,
        &book,
        max_volume,
        extension,
    ))
}

// ---------------------------------------------------------------------------
// DELETE /telegram-monitor/books/:id  — dismiss (soft) or hard delete (?hard=true)
// ---------------------------------------------------------------------------

#[utoipa::path(
    delete, path = "/telegram-monitor/books/{id}",
    tag = "telegram-monitor",
    params(("id" = String, Path, description = "Book link UUID")),
    responses((status = 200), (status = 404), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn dismiss_book(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
    Query(query): Query<DismissBookQuery>,
) -> Result<Json<Value>, ApiError> {
    let current =
        sqlx::query_scalar::<_, String>("SELECT status FROM telegram_book_links WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("book link not found"))?;

    // Abort running/queued task before any DB change so the semaphore slot is freed immediately
    if current == "queued" || current == "downloading" {
        if let Some(handle) = state.telegram_abort_handles.lock().await.remove(&id) {
            handle.abort();
        }
    }

    if query.hard.unwrap_or(false) {
        sqlx::query("DELETE FROM telegram_book_links WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await?;
    } else {
        sqlx::query(
            "UPDATE telegram_book_links SET status = 'dismissed', updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&state.pool)
        .await?;

        // Also insert into release_blacklist so the book appears in "Releases masquées"
        let _ = sqlx::query(
            "INSERT INTO release_blacklist (title, indexer, series_name, tg_book_id) \
             SELECT tbl.filename, ts.channel_username, tbl.series_name, tbl.id \
             FROM telegram_book_links tbl \
             JOIN telegram_sources ts ON ts.id = tbl.source_id \
             WHERE tbl.id = $1 \
             ON CONFLICT (title) DO UPDATE SET tg_book_id = EXCLUDED.tg_book_id",
        )
        .bind(id)
        .execute(&state.pool)
        .await;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/live-search  — live Telegram search + DB fallback
// ---------------------------------------------------------------------------

pub async fn search_books_by_pattern(
    pool: &sqlx::PgPool,
    pattern: &str,
) -> Result<Vec<TelegramSearchResultDto>, ApiError> {
    refresh_reimportable_telegram_links(pool).await?;

    let rows = sqlx::query(
        "SELECT b.id, s.channel_username, b.filename, b.file_size, b.volume_number, \
                b.series_name, b.status, b.created_at \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.status != 'dismissed' \
           AND b.series_name IS NOT NULL \
           AND norm_text(b.series_name) LIKE norm_text($1) \
         ORDER BY b.volume_number NULLS LAST, b.created_at DESC \
         LIMIT 200",
    )
    .bind(pattern)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| TelegramSearchResultDto {
            id: r.get::<Uuid, _>("id").to_string(),
            channel_username: r.get("channel_username"),
            filename: r.get("filename"),
            file_size: r.get("file_size"),
            volume_number: r.get("volume_number"),
            series_name: r.get("series_name"),
            status: r.get("status"),
            created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        })
        .collect())
}

#[utoipa::path(
    post, path = "/telegram-monitor/live-search",
    tag = "telegram-monitor",
    request_body = LiveSearchRequest,
    responses(
        (status = 200, body = Vec<TelegramSearchResultDto>),
        (status = 400, description = "Not configured or not authorized"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn live_search(
    State(state): State<AppState>,
    Json(body): Json<LiveSearchRequest>,
) -> Result<Json<Vec<TelegramSearchResultDto>>, ApiError> {
    use grammers_client::grammers_tl_types::enums::MessagesFilter;
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "pdf", "epub", "zip"];

    let query = body.query.trim().to_string();
    if query.is_empty() {
        return Err(ApiError::bad_request("Query cannot be empty"));
    }

    let (api_id, api_hash, _phone, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Telegram not configured"))?;

    let session_bytes =
        session_bytes.ok_or_else(|| ApiError::bad_request("Telegram not authorized"))?;

    let session = Session::load(&session_bytes)
        .map_err(|e| ApiError::internal(format!("Session load: {e}")))?;

    let client = Client::connect(Config {
        session,
        api_id: api_id as i32,
        api_hash,
        params: Default::default(),
    })
    .await
    .map_err(|e| ApiError::internal(format!("Telegram connect: {e}")))?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get::<Option<Uuid>, _>("library_id")))
    .collect();

    let mut new_books = 0usize;

    for (source_id, username, library_id) in &sources {
        let chat = match client.resolve_username(username).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                error!("live_search: channel @{username} not found");
                continue;
            }
            Err(e) => {
                error!("live_search: resolve @{username}: {e}");
                continue;
            }
        };

        let mut iter = client
            .search_messages(&chat)
            .query(&query)
            .filter(MessagesFilter::InputMessagesFilterDocument);

        loop {
            match iter.next().await {
                Ok(Some(message)) => {
                    if let Err(e) = insert_document_message(
                        &state.pool,
                        *source_id,
                        *library_id,
                        BOOK_EXTENSIONS,
                        &message,
                        &mut new_books,
                    )
                    .await
                    {
                        error!("live_search insert @{username}: {e}");
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    error!("live_search iter @{username}: {e}");
                    break;
                }
            }
        }
    }

    let updated_bytes = client.session().save();
    save_session_to_db(&state.pool, updated_bytes).await?;

    info!(
        "Telegram live search '{query}': {new_books} new books across {} sources",
        sources.len()
    );

    let pattern = format!("%{}%", query);
    let results = search_books_by_pattern(&state.pool, &pattern).await?;
    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// GET /telegram-monitor/search?q=...  — local DB search only (fast fallback)
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/search",
    tag = "telegram-monitor",
    params(("q" = Option<String>, Query, description = "Series name query")),
    responses((status = 200, body = Vec<TelegramSearchResultDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn search_available_books(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<SearchAvailableQuery>,
) -> Result<Json<Vec<TelegramSearchResultDto>>, ApiError> {
    let q = params.q.unwrap_or_default();
    if q.trim().is_empty() {
        return Ok(Json(vec![]));
    }
    let pattern = format!("%{}%", q);
    let results = search_books_by_pattern(&state.pool, &pattern).await?;
    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// AppState field for pending auth
// ---------------------------------------------------------------------------
