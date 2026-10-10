use axum::{extract::State, Json};
use sqlx::Row;
use tracing::{error, info};
use uuid::Uuid;

use crate::error::ApiError;
use crate::jobs::lifecycle::{complete_job, job_in_flight, JobScope};
use crate::state::AppState;

use super::auth::{load_tg_settings, save_session_to_db};
use super::channels::{do_sync, insert_document_message};

// ---------------------------------------------------------------------------
// Manual job trigger: enqueue a pending telegram_sync job
// ---------------------------------------------------------------------------

pub async fn start_incremental_sync_job(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'telegram_monitor' AND value->>'session_data' IS NOT NULL)",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !session_exists {
        return Err(ApiError::bad_request("Telegram not authorized"));
    }

    let existing = job_in_flight(
        &state.pool,
        JobScope::Global,
        &["telegram_sync_incremental"],
    )
    .await?;

    if let Some(existing_id) = existing {
        return Ok(Json(serde_json::json!({
            "id": existing_id.to_string(),
            "status": "already_running",
        })));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'telegram_sync_incremental', 'pending')",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "pending",
    })))
}

pub async fn start_sync_job(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'telegram_monitor' AND value->>'session_data' IS NOT NULL)",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !session_exists {
        return Err(ApiError::bad_request("Telegram not authorized"));
    }

    let existing = job_in_flight(&state.pool, JobScope::Global, &["telegram_sync"]).await?;

    if let Some(existing_id) = existing {
        return Ok(Json(serde_json::json!({
            "id": existing_id.to_string(),
            "status": "already_running",
        })));
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'telegram_sync', 'pending')",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "id": job_id.to_string(),
        "status": "pending",
    })))
}

// ---------------------------------------------------------------------------
// Background job: process_telegram_sync (called by job poller)
// ---------------------------------------------------------------------------

pub async fn process_telegram_sync(pool: &sqlx::PgPool, job_id: Uuid) -> Result<(), String> {
    info!("[TG_SYNC] Starting telegram sync job {job_id}");

    let (api_id, api_hash, _, session_bytes) = load_tg_settings(pool)
        .await
        .ok_or_else(|| "Telegram not configured".to_string())?;

    let session_bytes = session_bytes.ok_or_else(|| "Telegram not authorized".to_string())?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get("library_id")))
    .collect();

    if sources.is_empty() {
        complete_job(
            pool,
            job_id,
            serde_json::json!({ "message": "No sources configured", "synced": 0, "new_books": 0 }),
        )
        .await
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let r = do_sync(
        pool.clone(),
        api_id as i32,
        api_hash,
        session_bytes,
        sources,
        Some(job_id),
    )
    .await
    .map_err(|e| format!("{e:?}"))?;

    info!(
        "[TG_SYNC] Job {job_id} complete: {} messages, {} new books, {} series searched",
        r.synced, r.new_books, r.series_searched
    );

    // Which series matched a local series (current catalog state)
    let matched_series: Vec<serde_json::Value> = sqlx::query(
        "SELECT b.series_name AS telegram_name, s.id AS series_id, s.name AS series_name, \
                COUNT(*) AS book_count \
         FROM telegram_book_links b \
         JOIN telegram_sources src ON src.id = b.source_id \
         JOIN series s ON norm_text(s.name) = norm_text(b.series_name) \
           AND s.library_id = COALESCE(b.library_id, src.library_id) \
         WHERE b.status = 'available' \
         GROUP BY b.series_name, s.id, s.name \
         ORDER BY b.series_name",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .iter()
    .map(|row| {
        serde_json::json!({
            "telegram_name": row.get::<String, _>("telegram_name"),
            "series_id": row.get::<Uuid, _>("series_id").to_string(),
            "series_name": row.get::<String, _>("series_name"),
            "book_count": row.get::<i64, _>("book_count"),
        })
    })
    .collect();

    let stats = {
        // Merge counts and extracted names for series that appear across multiple sources
        let mut deduped: std::collections::BTreeMap<
            &str,
            (usize, std::collections::BTreeSet<String>),
        > = std::collections::BTreeMap::new();
        for (name, count, extracted) in &r.series_results {
            let entry = deduped.entry(name.as_str()).or_default();
            entry.0 += count;
            entry.1.extend(extracted.iter().cloned());
        }
        let all_series: Vec<serde_json::Value> = deduped
            .iter()
            .map(|(name, (count, extracted))| {
                serde_json::json!({
                    "series_name": name,
                    "book_count": count,
                    "extracted_names": extracted.iter().collect::<Vec<_>>(),
                })
            })
            .collect();
        serde_json::json!({
            "synced": r.synced,
            "new_books": r.new_books,
            "series_searched": r.series_searched,
            "all_series": all_series,
            "matched_series": matched_series,
        })
    };

    complete_job(pool, job_id, stats)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Background job: process_telegram_sync_incremental (called by job poller)
// ---------------------------------------------------------------------------

pub async fn process_telegram_sync_incremental(
    pool: &sqlx::PgPool,
    job_id: Uuid,
) -> Result<(), String> {
    info!("[TG_SYNC_INC] Starting incremental telegram sync job {job_id}");

    let (api_id, api_hash, _, session_bytes) = load_tg_settings(pool)
        .await
        .ok_or_else(|| "Telegram not configured".to_string())?;

    let session_bytes = session_bytes.ok_or_else(|| "Telegram not authorized".to_string())?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get("library_id")))
    .collect();

    if sources.is_empty() {
        complete_job(
            pool,
            job_id,
            serde_json::json!({ "message": "No sources configured", "new_books": 0 }),
        )
        .await
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let total = sources.len() as i32;
    let _ = sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
        .bind(job_id)
        .bind(total)
        .execute(pool)
        .await;

    let started_at = chrono::Utc::now();

    let (new_books, sources_scanned, per_source) = do_incremental_sync(
        pool.clone(),
        api_id as i32,
        api_hash,
        session_bytes,
        sources,
        job_id,
    )
    .await
    .map_err(|e| format!("{e:?}"))?;

    info!("[TG_SYNC_INC] Job {job_id} complete: {new_books} new books across {sources_scanned} sources");

    // Fetch the books actually added during this run (by creation timestamp)
    let recent_rows = sqlx::query(
        "SELECT b.filename, b.series_name, b.volume_number, s.channel_username \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.created_at >= $1 \
         ORDER BY b.created_at DESC \
         LIMIT 200",
    )
    .bind(started_at)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let recent_books: Vec<serde_json::Value> = recent_rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "filename":       r.get::<String, _>("filename"),
                "series_name":    r.get::<Option<String>, _>("series_name"),
                "volume_number":  r.get::<Option<i32>, _>("volume_number"),
                "channel":        r.get::<String, _>("channel_username"),
            })
        })
        .collect();

    // Only notify about files that match a series in the configured library.
    // Keep `recent_books` above unfiltered so the job report remains a complete
    // record of everything Telegram discovered.
    let matched_rows = sqlx::query(
        "SELECT b.filename, b.series_name, b.volume_number, s.channel_username \
         FROM telegram_book_links b \
         JOIN telegram_sources s ON s.id = b.source_id \
         WHERE b.created_at >= $1 \
           AND b.series_name IS NOT NULL \
           AND EXISTS ( \
             SELECT 1 FROM series sr \
             WHERE sr.library_id = COALESCE(b.library_id, s.library_id) \
               AND norm_text(sr.name) = norm_text(b.series_name) \
           ) \
         ORDER BY b.created_at DESC \
         LIMIT 200",
    )
    .bind(started_at)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut matched_per_source = std::collections::BTreeMap::<String, usize>::new();
    let notif_items: Vec<(String, Option<i32>)> = matched_rows
        .iter()
        .map(|r| {
            let label = r.get::<String, _>("series_name");
            let vol = r.get::<Option<i32>, _>("volume_number");
            *matched_per_source
                .entry(r.get::<String, _>("channel_username"))
                .or_default() += 1;
            (label, vol)
        })
        .collect();

    let sources_json: Vec<serde_json::Value> = per_source
        .iter()
        .map(|(username, count)| serde_json::json!({ "username": username, "new_books": count }))
        .collect();

    complete_job(
        pool,
        job_id,
        serde_json::json!({
            "new_books": new_books,
            "sources_scanned": sources_scanned,
            "sources": sources_json,
            "recent_books": recent_books,
        }),
    )
    .await
    .map_err(|e| e.to_string())?;

    notifications::notify(
        pool.clone(),
        notifications::NotificationEvent::TelegramSyncIncrementalCompleted {
            matched_books: notif_items.len(),
            sources_scanned,
            per_source: matched_per_source.into_iter().collect(),
            new_items: notif_items,
        },
    );

    Ok(())
}

pub async fn do_incremental_sync(
    pool: sqlx::PgPool,
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    sources: Vec<(Uuid, String, Option<Uuid>)>,
    job_id: Uuid,
) -> Result<(usize, usize, Vec<(String, usize)>), ApiError> {
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "pdf", "epub", "zip"];

    let session = Session::load(&session_bytes)
        .map_err(|e| ApiError::internal(format!("Session load: {e}")))?;

    let client = Client::connect(Config {
        session,
        api_id,
        api_hash,
        params: Default::default(),
    })
    .await
    .map_err(|e| ApiError::internal(format!("Connect: {e}")))?;

    let mut total_new = 0usize;
    let mut sources_scanned = 0usize;
    let mut per_source: Vec<(String, usize)> = Vec::new();
    let total = sources.len() as i32;

    for (idx, (source_id, username, library_id)) in sources.iter().enumerate() {
        let pct = (idx as i32 * 100 / total.max(1)) as i16;
        let label = format!("@{username} (incremental)");
        let _ = sqlx::query(
            "UPDATE index_jobs SET processed_files = $2, progress_percent = $3, current_file = $4 WHERE id = $1",
        )
        .bind(job_id).bind(idx as i32).bind(pct).bind(&label)
        .execute(&pool).await;

        // Highest message_id already stored for this source — scan only newer messages
        let max_known: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(message_id) FROM telegram_book_links WHERE source_id = $1",
        )
        .bind(source_id)
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();

        let chat = match client.resolve_username(username).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                error!("[TG_SYNC_INC] Channel @{username} not found");
                continue;
            }
            Err(e) => {
                error!("[TG_SYNC_INC] Resolve @{username}: {e}");
                continue;
            }
        };

        let mut new_books = 0usize;
        let mut iter = client.iter_messages(&chat);

        loop {
            match iter.next().await {
                Ok(Some(message)) => {
                    let msg_id = message.id() as i64;
                    if let Some(max_id) = max_known {
                        if msg_id <= max_id {
                            break;
                        }
                    }
                    if let Err(e) = insert_document_message(
                        &pool,
                        *source_id,
                        *library_id,
                        BOOK_EXTENSIONS,
                        &message,
                        &mut new_books,
                    )
                    .await
                    {
                        error!("[TG_SYNC_INC] Insert @{username} msg {msg_id}: {e}");
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    error!("[TG_SYNC_INC] iter @{username}: {e}");
                    break;
                }
            }
        }

        total_new += new_books;
        sources_scanned += 1;
        per_source.push((username.clone(), new_books));
        info!(
            "[TG_SYNC_INC] @{username}: {new_books} new books (since msg id {})",
            max_known.unwrap_or(0)
        );
    }

    let updated_bytes = client.session().save();
    save_session_to_db(&pool, updated_bytes).await?;

    Ok((total_new, sources_scanned, per_source))
}
