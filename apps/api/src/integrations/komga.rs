use std::collections::HashMap;

use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use stripstream_core::http::build_http_client;

use crate::{error::ApiError, state::AppState};

// ─── Komga API types ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct KomgaBooksResponse {
    content: Vec<KomgaBook>,
    #[serde(rename = "totalPages")]
    total_pages: i32,
    number: i32,
}

#[derive(Deserialize)]
struct KomgaBook {
    name: String,
    #[serde(rename = "seriesTitle")]
    series_title: String,
    metadata: KomgaBookMetadata,
}

#[derive(Deserialize)]
struct KomgaBookMetadata {
    title: String,
}

// ─── Request / Response ──────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct KomgaSyncRequest {
    pub url: String,
    pub username: String,
    pub password: String,
    #[schema(value_type = String)]
    pub user_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct KomgaSyncResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub komga_url: String,
    #[schema(value_type = Option<String>)]
    pub user_id: Option<Uuid>,
    pub total_komga_read: i64,
    pub matched: i64,
    pub already_read: i64,
    pub newly_marked: i64,
    pub matched_books: Vec<String>,
    pub newly_marked_books: Vec<String>,
    pub unmatched: Vec<String>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct KomgaSyncReportSummary {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub komga_url: String,
    #[schema(value_type = Option<String>)]
    pub user_id: Option<Uuid>,
    pub total_komga_read: i64,
    pub matched: i64,
    pub already_read: i64,
    pub newly_marked: i64,
    pub unmatched_count: i32,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// Sync read books from a Komga server
#[utoipa::path(
    post,
    path = "/komga/sync",
    tag = "komga",
    request_body = KomgaSyncRequest,
    responses(
        (status = 200, body = KomgaSyncResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Komga connection or sync error"),
    ),
    security(("Bearer" = []))
)]
pub async fn sync_komga_read_books(
    State(state): State<AppState>,
    Json(body): Json<KomgaSyncRequest>,
) -> Result<Json<KomgaSyncResponse>, ApiError> {
    let url = body.url.trim_end_matches('/').to_string();
    if url.is_empty() {
        return Err(ApiError::bad_request("url is required"));
    }

    // Build HTTP client with basic auth
    let client = build_http_client(std::time::Duration::from_secs(30))
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    // Paginate through all READ books from Komga
    let mut komga_books: Vec<(String, String)> = Vec::new(); // (series_title, title)
    let mut page = 0;
    let page_size = 100;
    let max_pages = 500;

    loop {
        let resp = client
            .post(format!("{url}/api/v1/books/list?page={page}&size={page_size}"))
            .basic_auth(&body.username, Some(&body.password))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "condition": { "readStatus": { "operator": "is", "value": "READ" } } }))
            .send()
            .await
            .map_err(|e| ApiError::internal(format!("Komga request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ApiError::internal(format!(
                "Komga returned {status}: {text}"
            )));
        }

        let data: KomgaBooksResponse = resp
            .json()
            .await
            .map_err(|e| ApiError::internal(format!("Failed to parse Komga response: {e}")))?;

        for book in &data.content {
            let title = if !book.metadata.title.is_empty() {
                &book.metadata.title
            } else {
                &book.name
            };
            komga_books.push((book.series_title.clone(), title.clone()));
        }

        if data.number >= data.total_pages - 1 || page >= max_pages {
            break;
        }
        page += 1;
    }

    let total_komga_read = komga_books.len() as i64;

    // Build local lookup maps
    let rows = sqlx::query(
        "SELECT id, title, COALESCE(series, '') as series, LOWER(title) as title_lower, LOWER(COALESCE(series, '')) as series_lower FROM books",
    )
    .fetch_all(&state.pool)
    .await?;

    type BookEntry = (Uuid, String, String);
    // Primary: (series_lower, title_lower) -> Vec<(Uuid, title, series)>
    let mut primary_map: HashMap<(String, String), Vec<BookEntry>> = HashMap::new();
    // Secondary: title_lower -> Vec<(Uuid, title, series)>
    let mut secondary_map: HashMap<String, Vec<BookEntry>> = HashMap::new();

    for row in &rows {
        let id: Uuid = row.get("id");
        let title: String = row.get("title");
        let series: String = row.get("series");
        let title_lower: String = row.get("title_lower");
        let series_lower: String = row.get("series_lower");
        let entry = (id, title, series);

        primary_map
            .entry((series_lower, title_lower.clone()))
            .or_default()
            .push(entry.clone());
        secondary_map.entry(title_lower).or_default().push(entry);
    }

    // Match Komga books to local books
    let mut matched_entries: Vec<(Uuid, String)> = Vec::new(); // (id, display_title)
    let mut unmatched: Vec<String> = Vec::new();

    for (series_title, title) in &komga_books {
        let title_lower = title.to_lowercase();
        let series_lower = series_title.to_lowercase();

        let found =
            if let Some(entries) = primary_map.get(&(series_lower.clone(), title_lower.clone())) {
                Some(entries)
            } else {
                secondary_map.get(&title_lower)
            };

        if let Some(entries) = found {
            for (id, local_title, local_series) in entries {
                let display = if local_series.is_empty() {
                    local_title.clone()
                } else {
                    format!("{local_series} - {local_title}")
                };
                matched_entries.push((*id, display));
            }
        } else if series_title.is_empty() {
            unmatched.push(title.clone());
        } else {
            unmatched.push(format!("{series_title} - {title}"));
        }
    }

    // Deduplicate by ID
    matched_entries.sort_by_key(|entry| entry.0);
    matched_entries.dedup_by(|a, b| a.0 == b.0);

    let matched_ids: Vec<Uuid> = matched_entries.iter().map(|(id, _)| *id).collect();
    let matched = matched_ids.len() as i64;
    let mut already_read: i64 = 0;
    let mut already_read_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    if !matched_ids.is_empty() {
        // Get already-read book IDs for this user
        let ar_rows = sqlx::query(
            "SELECT book_id FROM book_reading_progress WHERE book_id = ANY($1) AND user_id = $2 AND status = 'read'",
        )
        .bind(&matched_ids)
        .bind(body.user_id)
        .fetch_all(&state.pool)
        .await?;

        for row in &ar_rows {
            already_read_ids.insert(row.get("book_id"));
        }
        already_read = already_read_ids.len() as i64;

        // Bulk upsert all matched books as read for this user
        sqlx::query(
            r#"
            INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
            SELECT unnest($1::uuid[]), $2, 'read', NULL, NOW(), NOW()
            ON CONFLICT (book_id, user_id) DO UPDATE
              SET status = 'read',
                  current_page = NULL,
                  last_read_at = NOW(),
                  updated_at = NOW()
              WHERE book_reading_progress.status != 'read'
            "#,
        )
        .bind(&matched_ids)
        .bind(body.user_id)
        .execute(&state.pool)
        .await?;
    }

    let newly_marked = matched - already_read;

    // Build matched_books and newly_marked_books lists
    let mut newly_marked_books: Vec<String> = Vec::new();
    let mut matched_books: Vec<String> = Vec::new();
    for (id, title) in &matched_entries {
        if !already_read_ids.contains(id) {
            newly_marked_books.push(title.clone());
        }
        matched_books.push(title.clone());
    }
    // Sort: newly marked first, then alphabetical
    let newly_marked_set: std::collections::HashSet<&str> =
        newly_marked_books.iter().map(|s| s.as_str()).collect();
    matched_books.sort_by(|a, b| {
        let a_new = newly_marked_set.contains(a.as_str());
        let b_new = newly_marked_set.contains(b.as_str());
        b_new.cmp(&a_new).then(a.cmp(b))
    });
    newly_marked_books.sort();

    // Save sync report
    let unmatched_json = serde_json::to_value(&unmatched).unwrap_or_default();
    let matched_books_json = serde_json::to_value(&matched_books).unwrap_or_default();
    let newly_marked_books_json = serde_json::to_value(&newly_marked_books).unwrap_or_default();
    let report_row = sqlx::query(
        r#"
        INSERT INTO komga_sync_reports (komga_url, user_id, total_komga_read, matched, already_read, newly_marked, matched_books, newly_marked_books, unmatched)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, created_at
        "#,
    )
    .bind(&url)
    .bind(body.user_id)
    .bind(total_komga_read)
    .bind(matched)
    .bind(already_read)
    .bind(newly_marked)
    .bind(&matched_books_json)
    .bind(&newly_marked_books_json)
    .bind(&unmatched_json)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(KomgaSyncResponse {
        id: report_row.get("id"),
        komga_url: url,
        user_id: Some(body.user_id),
        total_komga_read,
        matched,
        already_read,
        newly_marked,
        matched_books,
        newly_marked_books,
        unmatched,
        created_at: report_row.get("created_at"),
    }))
}

/// List Komga sync reports (most recent first)
#[utoipa::path(
    get,
    path = "/komga/reports",
    tag = "komga",
    responses(
        (status = 200, body = Vec<KomgaSyncReportSummary>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_sync_reports(
    State(state): State<AppState>,
) -> Result<Json<Vec<KomgaSyncReportSummary>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT id, komga_url, user_id, total_komga_read, matched, already_read, newly_marked,
               jsonb_array_length(unmatched) as unmatched_count, created_at
        FROM komga_sync_reports
        ORDER BY created_at DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let reports: Vec<KomgaSyncReportSummary> = rows
        .iter()
        .map(|row| KomgaSyncReportSummary {
            id: row.get("id"),
            komga_url: row.get("komga_url"),
            user_id: row.get("user_id"),
            total_komga_read: row.get("total_komga_read"),
            matched: row.get("matched"),
            already_read: row.get("already_read"),
            newly_marked: row.get("newly_marked"),
            unmatched_count: row.get("unmatched_count"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(reports))
}

/// Get a specific sync report with full unmatched list
#[utoipa::path(
    get,
    path = "/komga/reports/{id}",
    tag = "komga",
    params(("id" = String, Path, description = "Report UUID")),
    responses(
        (status = 200, body = KomgaSyncResponse),
        (status = 404, description = "Report not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_sync_report(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<KomgaSyncResponse>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, komga_url, user_id, total_komga_read, matched, already_read, newly_marked, matched_books, newly_marked_books, unmatched, created_at
        FROM komga_sync_reports
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("report not found"))?;

    let matched_books_json: serde_json::Value = row
        .try_get("matched_books")
        .unwrap_or(serde_json::Value::Array(vec![]));
    let matched_books: Vec<String> = serde_json::from_value(matched_books_json).unwrap_or_default();
    let newly_marked_books_json: serde_json::Value = row
        .try_get("newly_marked_books")
        .unwrap_or(serde_json::Value::Array(vec![]));
    let newly_marked_books: Vec<String> =
        serde_json::from_value(newly_marked_books_json).unwrap_or_default();
    let unmatched_json: serde_json::Value = row.get("unmatched");
    let unmatched: Vec<String> = serde_json::from_value(unmatched_json).unwrap_or_default();

    Ok(Json(KomgaSyncResponse {
        id: row.get("id"),
        komga_url: row.get("komga_url"),
        user_id: row.get("user_id"),
        total_komga_read: row.get("total_komga_read"),
        matched: row.get("matched"),
        already_read: row.get("already_read"),
        newly_marked: row.get("newly_marked"),
        matched_books,
        newly_marked_books,
        unmatched,
        created_at: row.get("created_at"),
    }))
}
