use axum::extract::{Path as AxumPath, State};
use axum::Json;
use chrono::Utc;
use serde_json::Value;
use sqlx::Row;
use tracing::{error, info};
use uuid::Uuid;

use parsers::extract_volume;

use crate::{error::ApiError, state::AppState};

use super::auth::{load_tg_settings, save_session_to_db};
use super::types::*;

#[utoipa::path(
    get, path = "/telegram-monitor/channels",
    tag = "telegram-monitor",
    params(("q" = Option<String>, Query, description = "Search query")),
    responses((status = 200, body = Vec<ChannelSuggestion>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn search_channels(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ChannelSuggestion>>, ApiError> {
    let q = params
        .get("q")
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let (api_id, api_hash, _, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Not configured"))?;
    let session_bytes = session_bytes.ok_or_else(|| ApiError::bad_request("Not authorized"))?;

    let results = do_search_channels(api_id as i32, api_hash, session_bytes, q).await?;
    Ok(Json(results))
}

pub async fn do_search_channels(
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    query: String,
) -> Result<Vec<ChannelSuggestion>, ApiError> {
    use grammers_client::types::Chat;
    use grammers_client::{Client, Config};
    use grammers_session::Session;

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

    let mut results = Vec::new();
    let mut dialogs = client.iter_dialogs();

    while let Some(dialog) = dialogs
        .next()
        .await
        .map_err(|e| ApiError::internal(format!("iter dialogs: {e}")))?
    {
        let chat = dialog.chat().clone();
        let title = chat.name().to_string();

        let (username, kind) = match &chat {
            Chat::Channel(c) => (c.username().map(str::to_string), "channel"),
            Chat::Group(_) => (None, "group"),
            Chat::User(_) => continue,
        };

        let matches = query.is_empty()
            || title.to_lowercase().contains(&query)
            || username
                .as_deref()
                .map(|u| u.to_lowercase().contains(&query))
                .unwrap_or(false);

        if matches {
            results.push(ChannelSuggestion {
                username,
                title,
                kind: kind.to_string(),
            });
        }

        if results.len() >= 30 {
            break;
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Sources CRUD
// ---------------------------------------------------------------------------

#[utoipa::path(
    get, path = "/telegram-monitor/sources",
    tag = "telegram-monitor",
    responses((status = 200, body = Vec<TelegramSourceDto>), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<TelegramSourceDto>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, channel_username, channel_title, library_id, enabled, created_at \
         FROM telegram_sources ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?;

    let sources = rows
        .iter()
        .map(|r| TelegramSourceDto {
            id: r.get::<Uuid, _>("id").to_string(),
            channel_username: r.get("channel_username"),
            channel_title: r.get("channel_title"),
            library_id: r
                .get::<Option<Uuid>, _>("library_id")
                .map(|u| u.to_string()),
            enabled: r.get("enabled"),
            created_at: r.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        })
        .collect();

    Ok(Json(sources))
}

#[utoipa::path(
    post, path = "/telegram-monitor/sources",
    tag = "telegram-monitor",
    request_body = AddSourceRequest,
    responses((status = 200, body = TelegramSourceDto), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn add_source(
    State(state): State<AppState>,
    Json(body): Json<AddSourceRequest>,
) -> Result<Json<TelegramSourceDto>, ApiError> {
    let username = body
        .channel_username
        .trim()
        .trim_start_matches('@')
        .to_string();
    let library_id = body
        .library_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| ApiError::bad_request("Invalid library_id"))?;

    let row = sqlx::query(
        "INSERT INTO telegram_sources (channel_username, library_id) VALUES ($1, $2) \
         ON CONFLICT (channel_username) DO UPDATE SET library_id = $2, enabled = true, updated_at = NOW() \
         RETURNING id, channel_username, channel_title, library_id, enabled, created_at",
    )
    .bind(&username)
    .bind(library_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(TelegramSourceDto {
        id: row.get::<Uuid, _>("id").to_string(),
        channel_username: row.get("channel_username"),
        channel_title: row.get("channel_title"),
        library_id: row
            .get::<Option<Uuid>, _>("library_id")
            .map(|u| u.to_string()),
        enabled: row.get("enabled"),
        created_at: row
            .get::<chrono::DateTime<Utc>, _>("created_at")
            .to_rfc3339(),
    }))
}

#[utoipa::path(
    delete, path = "/telegram-monitor/sources/{id}",
    tag = "telegram-monitor",
    params(("id" = String, Path, description = "Source UUID")),
    responses((status = 200), (status = 404), (status = 401)),
    security(("Bearer" = []))
)]
pub async fn delete_source(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let res = sqlx::query("DELETE FROM telegram_sources WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err(ApiError::not_found("source not found"));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// POST /telegram-monitor/sync  — sync all enabled sources
// ---------------------------------------------------------------------------

#[utoipa::path(
    post, path = "/telegram-monitor/sync",
    tag = "telegram-monitor",
    responses(
        (status = 200, body = SyncResult),
        (status = 400, description = "Not configured or not authorized"),
        (status = 401),
    ),
    security(("Bearer" = []))
)]
pub async fn sync_sources(State(state): State<AppState>) -> Result<Json<SyncResult>, ApiError> {
    let (api_id, api_hash, _, session_bytes) = load_tg_settings(&state.pool)
        .await
        .ok_or_else(|| ApiError::bad_request("Telegram monitor not configured"))?;

    let session_bytes = session_bytes
        .ok_or_else(|| ApiError::bad_request("Not authorized — complete auth first"))?;

    let sources: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query(
        "SELECT id, channel_username, library_id FROM telegram_sources WHERE enabled = true",
    )
    .fetch_all(&state.pool)
    .await?
    .iter()
    .map(|r| (r.get("id"), r.get("channel_username"), r.get("library_id")))
    .collect();

    if sources.is_empty() {
        return Ok(Json(SyncResult {
            synced: 0,
            new_books: 0,
            series_searched: 0,
            series_results: vec![],
        }));
    }

    let result = do_sync(
        state.pool.clone(),
        api_id as i32,
        api_hash,
        session_bytes,
        sources,
        None,
    )
    .await?;

    Ok(Json(result))
}

pub async fn do_sync(
    pool: sqlx::PgPool,
    api_id: i32,
    api_hash: String,
    session_bytes: Vec<u8>,
    sources: Vec<(Uuid, String, Option<Uuid>)>,
    job_id: Option<Uuid>,
) -> Result<SyncResult, ApiError> {
    use grammers_client::{Client, Config};
    use grammers_session::Session;

    const BOOK_EXTENSIONS: &[&str] = &["cbz", "cbr", "pdf", "epub", "zip"];

    // Pre-load filtered series per source before opening Telegram connection.
    // Only series with an approved metadata link AND at least one missing volume
    // (same rule as download detection / Prowlarr).
    let mut source_work: Vec<(Uuid, String, Option<Uuid>, Vec<String>)> = Vec::new();
    for (source_id, username, library_id) in sources {
        let series_names: Vec<String> = if let Some(lib_id) = library_id {
            sqlx::query_scalar::<_, String>(
                "SELECT DISTINCT s.name \
                 FROM series s \
                 JOIN external_metadata_links eml \
                   ON eml.series_id = s.id AND eml.status = 'approved' AND eml.library_id = $1 \
                 WHERE s.library_id = $1 \
                   AND EXISTS ( \
                       SELECT 1 FROM external_book_metadata ebm \
                       WHERE ebm.link_id = eml.id AND ebm.book_id IS NULL \
                   ) \
                   AND NOT EXISTS ( \
                       SELECT 1 FROM books b \
                       WHERE b.series_id = s.id AND b.volume_type = 'integral' \
                   ) \
                 ORDER BY s.name",
            )
            .bind(lib_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        } else {
            vec![]
        };

        if series_names.is_empty() {
            info!("Telegram sync @{username}: no eligible series (need metadata link + missing volumes), skipping");
        } else {
            source_work.push((source_id, username, library_id, series_names));
        }
    }

    let total_series = source_work
        .iter()
        .map(|(_, _, _, sn)| sn.len())
        .sum::<usize>() as i32;

    if let Some(jid) = job_id {
        let _ = sqlx::query("UPDATE index_jobs SET total_files = $2 WHERE id = $1")
            .bind(jid)
            .bind(total_series)
            .execute(&pool)
            .await;
    }

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

    let mut total_synced = 0usize;
    let mut total_new = 0usize;
    let mut total_series_searched = 0usize;
    let mut all_series_results: Vec<(String, usize, Vec<String>)> = Vec::new();
    let mut processed = 0i32;

    for (source_id, username, library_id, series_names) in source_work {
        total_series_searched += series_names.len();

        match sync_one_source(
            &client,
            &pool,
            source_id,
            &username,
            library_id,
            BOOK_EXTENSIONS,
            &series_names,
            job_id,
            &mut processed,
            total_series,
        )
        .await
        {
            Ok((synced, new_books, series_results)) => {
                total_synced += synced;
                total_new += new_books;
                all_series_results.extend(series_results);
            }
            Err(e) => error!("Telegram sync error for {username}: {e}"),
        }
    }

    let updated_bytes = client.session().save();
    save_session_to_db(&pool, updated_bytes).await?;

    Ok(SyncResult {
        synced: total_synced,
        new_books: total_new,
        series_searched: total_series_searched,
        series_results: all_series_results,
    })
}

pub async fn insert_document_message(
    pool: &sqlx::PgPool,
    source_id: Uuid,
    library_id: Option<Uuid>,
    extensions: &[&str],
    message: &grammers_client::types::Message,
    new_books: &mut usize,
) -> anyhow::Result<()> {
    use grammers_client::types::Media;

    if let Some(Media::Document(doc)) = message.media() {
        let filename = doc.name().to_string();
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !extensions.contains(&ext.as_str()) {
            return Ok(());
        }

        let msg_id = message.id() as i64;
        let text = message.text();
        let msg_text = if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        };
        let series_name = parsers::extract_series_name_from_filename(&filename);
        let series_name = if series_name.is_empty() {
            None
        } else {
            Some(series_name)
        };
        let volume_number = extract_volume(&filename);

        let inserted = sqlx::query(
            "INSERT INTO telegram_book_links \
             (source_id, message_id, filename, file_size, mime_type, message_text, library_id, series_name, volume_number) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (source_id, message_id) DO UPDATE SET volume_number = EXCLUDED.volume_number WHERE telegram_book_links.volume_number IS NULL",
        )
        .bind(source_id)
        .bind(msg_id)
        .bind(&filename)
        .bind(doc.size())
        .bind(doc.mime_type())
        .bind(msg_text)
        .bind(library_id)
        .bind(series_name)
        .bind(volume_number)
        .execute(pool)
        .await?;

        if inserted.rows_affected() > 0 {
            *new_books += 1;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn sync_one_source(
    client: &grammers_client::Client,
    pool: &sqlx::PgPool,
    source_id: Uuid,
    username: &str,
    library_id: Option<Uuid>,
    extensions: &[&str],
    series_names: &[String],
    job_id: Option<Uuid>,
    processed: &mut i32,
    total: i32,
) -> anyhow::Result<(usize, usize, Vec<(String, usize, Vec<String>)>)> {
    use grammers_client::grammers_tl_types::enums::MessagesFilter;

    let chat = match client.resolve_username(username).await? {
        Some(c) => c,
        None => anyhow::bail!("Channel not found: {username}"),
    };

    use grammers_client::types::Media;
    use std::collections::BTreeSet;

    let mut synced = 0usize;
    let mut new_books = 0usize;
    let mut series_results: Vec<(String, usize, Vec<String>)> = Vec::new();

    for series_name in series_names {
        if let Some(jid) = job_id {
            let pct = (*processed * 100 / total.max(1)) as i16;
            let label = format!("@{username}: {series_name}");
            let _ = sqlx::query(
                "UPDATE index_jobs \
                 SET processed_files = $2, progress_percent = $3, current_file = $4 \
                 WHERE id = $1",
            )
            .bind(jid)
            .bind(*processed)
            .bind(pct)
            .bind(&label)
            .execute(pool)
            .await;
        }

        let mut iter = client
            .search_messages(&chat)
            .query(series_name)
            .filter(MessagesFilter::InputMessagesFilterDocument);

        let mut series_count = 0usize;
        let mut extracted_names: BTreeSet<String> = BTreeSet::new();
        while let Some(message) = iter.next().await? {
            synced += 1;
            series_count += 1;
            if let Some(Media::Document(doc)) = message.media() {
                let extracted = parsers::extract_series_name_from_filename(doc.name());
                if !extracted.is_empty() {
                    extracted_names.insert(extracted);
                }
            }
            insert_document_message(
                pool,
                source_id,
                library_id,
                extensions,
                &message,
                &mut new_books,
            )
            .await?;
        }
        if series_count > 0 {
            series_results.push((
                series_name.clone(),
                series_count,
                extracted_names.into_iter().collect(),
            ));
        }
        *processed += 1;
    }

    // Update channel title
    if let Some(title) = get_chat_title(client, username).await {
        sqlx::query(
            "UPDATE telegram_sources SET channel_title = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(title)
        .bind(source_id)
        .execute(pool)
        .await?;
    }

    info!("Telegram search @{username}: {synced} messages scanned, {new_books} new books ({} series with results)", series_results.len());
    Ok((synced, new_books, series_results))
}

pub async fn get_chat_title(client: &grammers_client::Client, username: &str) -> Option<String> {
    match client.resolve_username(username).await {
        Ok(Some(chat)) => Some(chat.name().to_string()),
        _ => None,
    }
}
