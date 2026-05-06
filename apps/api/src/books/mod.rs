pub mod pages;
pub mod rename;
pub mod thumbnails;

pub use pages::*;
pub use rename::*;
pub use thumbnails::*;

use axum::{extract::{Extension, Path, Query, State}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{auth::AuthUser, error::ApiError, index_jobs::IndexJobResponse, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct ListBooksQuery {
    /// Text search on title, series and author (case-insensitive, partial match)
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub kind: Option<String>,
    #[schema(value_type = Option<String>, example = "cbz")]
    pub format: Option<String>,
    #[schema(value_type = Option<String>)]
    pub series: Option<String>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    /// Filter by exact author name (matches in authors array or scalar author field)
    #[schema(value_type = Option<String>)]
    pub author: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
    /// Sort order: "title" (default) or "latest" (most recently added first)
    #[schema(value_type = Option<String>, example = "latest")]
    pub sort: Option<String>,
    /// Filter by metadata provider: "linked" (any provider), "unlinked" (no provider), or a specific provider name
    #[schema(value_type = Option<String>, example = "linked")]
    pub metadata_provider: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct BookItem {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub kind: String,
    pub format: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub authors: Vec<String>,
    pub series: Option<String>,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub language: Option<String>,
    pub page_count: Option<i32>,
    pub thumbnail_url: Option<String>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
    /// Reading status: "unread", "reading", or "read"
    pub reading_status: String,
    pub reading_current_page: Option<i32>,
    #[schema(value_type = Option<String>)]
    pub reading_last_read_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct BooksPage {
    pub items: Vec<BookItem>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Serialize, ToSchema)]
pub struct BookDetails {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub kind: String,
    pub title: String,
    pub author: Option<String>,
    pub authors: Vec<String>,
    pub series: Option<String>,
    #[schema(value_type = Option<String>)]
    pub series_id: Option<Uuid>,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub language: Option<String>,
    pub page_count: Option<i32>,
    pub thumbnail_url: Option<String>,
    pub file_path: Option<String>,
    pub file_format: Option<String>,
    pub file_parse_status: Option<String>,
    /// Reading status: "unread", "reading", or "read"
    pub reading_status: String,
    pub reading_current_page: Option<i32>,
    #[schema(value_type = Option<String>)]
    pub reading_last_read_at: Option<DateTime<Utc>>,
    pub summary: Option<String>,
    pub isbn: Option<String>,
    pub publish_date: Option<String>,
    /// Fields locked from external metadata sync
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_fields: Option<serde_json::Value>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

/// List books with optional filtering and pagination
#[utoipa::path(
    get,
    path = "/books",
    tag = "books",
    params(
        ("q" = Option<String>, Query, description = "Text search on title, series and author (case-insensitive, partial match)"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("kind" = Option<String>, Query, description = "Filter by book kind (cbz, cbr, pdf, epub)"),
        ("series" = Option<String>, Query, description = "Filter by series name (use 'unclassified' for books without series)"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default) or 'latest' (most recently added first)"),
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: 'linked' (any provider), 'unlinked' (no provider), or a specific provider name"),
    ),
    responses(
        (status = 200, body = BooksPage),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_books(
    State(state): State<AppState>,
    Query(query): Query<ListBooksQuery>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<BooksPage>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    // Parse reading_status CSV → Vec<String>
    let reading_statuses: Option<Vec<String>> = query.reading_status.as_deref().map(|s| {
        s.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect()
    });

    // Conditions partagées COUNT et DATA — $1=library_id $2=kind $3=format, puis optionnels
    let mut p: usize = 3;
    let series_cond = match query.series.as_deref() {
        Some("unclassified") => "AND b.series_id IS NULL".to_string(),
        Some(_) => { p += 1; format!("AND b.series_id = ${p}") }
        None => String::new(),
    };
    let rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND COALESCE(brp.status, 'unread') = ANY(${p})")
    } else { String::new() };
    let author_cond = if query.author.is_some() {
        p += 1; format!("AND (${p} = ANY(COALESCE(NULLIF(b.authors, '{{}}'), CASE WHEN b.author IS NOT NULL AND b.author != '' THEN ARRAY[b.author] ELSE ARRAY[]::text[] END)) OR (s.id IS NOT NULL AND ${p} = ANY(COALESCE(s.authors, ARRAY[]::text[]))))")
    } else { String::new() };
    let metadata_cond = match query.metadata_provider.as_deref() {
        Some("unlinked") => "AND eml.id IS NULL".to_string(),
        Some("linked") => "AND eml.id IS NOT NULL".to_string(),
        Some(_) => { p += 1; format!("AND eml.provider = ${p}") },
        None => String::new(),
    };
    let q_cond = if query.q.is_some() {
        p += 1; format!("AND (b.title ILIKE ${p} OR s.name ILIKE ${p} OR b.author ILIKE ${p})")
    } else { String::new() };
    p += 1;
    let uid_p = p;

    let metadata_links_cte = r#"
        metadata_links AS (
            SELECT DISTINCT ON (eml.series_id, eml.library_id)
                eml.series_id, eml.library_id, eml.provider, eml.id
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_id, eml.library_id, eml.created_at DESC
        )"#;

    let count_sql = format!(
        r#"WITH {metadata_links_cte}
           SELECT COUNT(*) FROM books b
           LEFT JOIN series s ON s.id = b.series_id
           LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${uid_p}::uuid IS NOT NULL AND brp.user_id = ${uid_p}
           LEFT JOIN metadata_links eml ON eml.series_id = b.series_id AND eml.library_id = b.library_id
           WHERE ($1::uuid IS NULL OR b.library_id = $1)
             AND ($2::text IS NULL OR b.kind = $2)
             AND ($3::text IS NULL OR b.format = $3)
             {series_cond}
             {rs_cond}
             {author_cond}
             {metadata_cond}
             {q_cond}"#
    );

    let order_clause = if query.sort.as_deref() == Some("latest") {
        "b.created_at DESC".to_string()
    } else {
        "b.volume NULLS LAST, REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''), COALESCE((REGEXP_MATCH(LOWER(b.title), '\\d+'))[1]::int, 0), b.title ASC".to_string()
    };

    // DATA: mêmes params filtre, puis $N+1=limit $N+2=offset
    let limit_p = p + 1;
    let offset_p = p + 2;
    let data_sql = format!(
        r#"
        WITH {metadata_links_cte}
        SELECT b.id, b.library_id, b.kind, b.format, b.title, b.author, b.authors, s.name AS series, b.volume, b.volume_type, b.language, b.page_count, b.thumbnail_path, b.updated_at,
               COALESCE(brp.status, 'unread') AS reading_status,
               brp.current_page AS reading_current_page,
               brp.last_read_at AS reading_last_read_at
        FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND ${uid_p}::uuid IS NOT NULL AND brp.user_id = ${uid_p}
        LEFT JOIN metadata_links eml ON eml.series_id = b.series_id AND eml.library_id = b.library_id
        WHERE ($1::uuid IS NULL OR b.library_id = $1)
          AND ($2::text IS NULL OR b.kind = $2)
          AND ($3::text IS NULL OR b.format = $3)
          {series_cond}
          {rs_cond}
          {author_cond}
          {metadata_cond}
          {q_cond}
        ORDER BY {order_clause}
        LIMIT ${limit_p} OFFSET ${offset_p}
        "#
    );

    let mut count_builder = sqlx::query(&count_sql)
        .bind(query.library_id)
        .bind(query.kind.as_deref())
        .bind(query.format.as_deref());
    let mut data_builder = sqlx::query(&data_sql)
        .bind(query.library_id)
        .bind(query.kind.as_deref())
        .bind(query.format.as_deref());

    if let Some(s) = query.series.as_deref() {
        if s != "unclassified" {
            let series_uuid: Uuid = s.parse().map_err(|_| ApiError::bad_request("invalid series id"))?;
            count_builder = count_builder.bind(series_uuid);
            data_builder = data_builder.bind(series_uuid);
        }
    }
    if let Some(ref statuses) = reading_statuses {
        count_builder = count_builder.bind(statuses.clone());
        data_builder = data_builder.bind(statuses.clone());
    }
    if let Some(ref author) = query.author {
        count_builder = count_builder.bind(author.clone());
        data_builder = data_builder.bind(author.clone());
    }
    if let Some(ref mp) = query.metadata_provider {
        if mp != "linked" && mp != "unlinked" {
            count_builder = count_builder.bind(mp.clone());
            data_builder = data_builder.bind(mp.clone());
        }
    }
    if let Some(ref q) = query.q {
        let pattern = format!("%{q}%");
        count_builder = count_builder.bind(pattern.clone());
        data_builder = data_builder.bind(pattern);
    }
    count_builder = count_builder.bind(user_id);
    data_builder = data_builder.bind(user_id).bind(limit).bind(offset);

    let (count_row, rows) = tokio::try_join!(
        count_builder.fetch_one(&state.pool),
        data_builder.fetch_all(&state.pool),
    )?;
    let total: i64 = count_row.get(0);

    let items: Vec<BookItem> = rows
        .iter()
        .map(|row| {
            let thumbnail_path: Option<String> = row.get("thumbnail_path");
            BookItem {
                id: row.get("id"),
                library_id: row.get("library_id"),
                kind: row.get("kind"),
                format: row.get("format"),
                title: row.get("title"),
                author: row.get("author"),
                authors: row.get::<Vec<String>, _>("authors"),
                series: row.get("series"),
                volume: row.get("volume"),
                volume_type: row.get("volume_type"),
                language: row.get("language"),
                page_count: row.get("page_count"),
                thumbnail_url: thumbnail_path.map(|_p| format!("/books/{}/thumbnail", row.get::<Uuid, _>("id"))),
                updated_at: row.get("updated_at"),
                reading_status: row.get("reading_status"),
                reading_current_page: row.get("reading_current_page"),
                reading_last_read_at: row.get("reading_last_read_at"),
            }
        })
        .collect();

    Ok(Json(BooksPage {
        items,
        total,
        page,
        limit,
    }))
}

/// Get detailed information about a specific book
#[utoipa::path(
    get,
    path = "/books/{id}",
    tag = "books",
    params(
        ("id" = String, Path, description = "Book UUID"),
    ),
    responses(
        (status = 200, body = BookDetails),
        (status = 404, description = "Book not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_book(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<BookDetails>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let row = sqlx::query(
        r#"
        SELECT b.id, b.library_id, b.kind, b.title, b.author, b.authors, s.name AS series, b.series_id, b.volume, b.volume_type, b.language, b.page_count, b.thumbnail_path, b.locked_fields, b.summary, b.isbn, b.publish_date, b.updated_at,
               bf.abs_path, bf.format, bf.parse_status,
               COALESCE(brp.status, 'unread') AS reading_status,
               brp.current_page AS reading_current_page,
               brp.last_read_at AS reading_last_read_at
        FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        LEFT JOIN LATERAL (
          SELECT abs_path, format, parse_status
          FROM book_files
          WHERE book_id = b.id
          ORDER BY updated_at DESC
          LIMIT 1
        ) bf ON TRUE
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
        WHERE b.id = $1
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("book not found"))?;
    let thumbnail_path: Option<String> = row.get("thumbnail_path");
    Ok(Json(BookDetails {
        id: row.get("id"),
        library_id: row.get("library_id"),
        kind: row.get("kind"),
        title: row.get("title"),
        author: row.get("author"),
        authors: row.get::<Vec<String>, _>("authors"),
        series: row.get("series"),
        series_id: row.get("series_id"),
        volume: row.get("volume"),
        volume_type: row.get("volume_type"),
        language: row.get("language"),
        page_count: row.get("page_count"),
        thumbnail_url: thumbnail_path.map(|_| format!("/books/{}/thumbnail", id)),
        file_path: row.get("abs_path"),
        file_format: row.get("format"),
        file_parse_status: row.get("parse_status"),
        reading_status: row.get("reading_status"),
        reading_current_page: row.get("reading_current_page"),
        reading_last_read_at: row.get("reading_last_read_at"),
        summary: row.get("summary"),
        isbn: row.get("isbn"),
        publish_date: row.get("publish_date"),
        locked_fields: Some(row.get::<serde_json::Value, _>("locked_fields")),
        updated_at: row.get("updated_at"),
    }))
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

pub(crate) use stripstream_core::paths::remap_libraries_path;
pub(crate) use stripstream_core::paths::unmap_libraries_path;

// ─── Convert CBR → CBZ ───────────────────────────────────────────────────────

/// Enqueue a CBR → CBZ conversion job for a single book
#[utoipa::path(
    post,
    path = "/books/{id}/convert",
    tag = "books",
    params(
        ("id" = String, Path, description = "Book UUID"),
    ),
    responses(
        (status = 200, body = IndexJobResponse),
        (status = 404, description = "Book not found"),
        (status = 409, description = "Book is not CBR, or target CBZ already exists"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn convert_book(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
) -> Result<Json<IndexJobResponse>, ApiError> {
    // Fetch book file info
    let row = sqlx::query(
        r#"
        SELECT b.id, bf.abs_path, bf.format
        FROM books b
        LEFT JOIN LATERAL (
            SELECT abs_path, format
            FROM book_files
            WHERE book_id = b.id
            ORDER BY updated_at DESC
            LIMIT 1
        ) bf ON TRUE
        WHERE b.id = $1
        "#,
    )
    .bind(book_id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("book not found"))?;
    let abs_path: Option<String> = row.get("abs_path");
    let format: Option<String> = row.get("format");

    if format.as_deref() != Some("cbr") {
        return Err(ApiError {
            status: axum::http::StatusCode::CONFLICT,
            message: "book is not in CBR format".to_string(),
        });
    }

    let abs_path = abs_path.ok_or_else(|| ApiError::not_found("book file path not found"))?;

    // Check for existing CBZ with same stem
    let physical_path = remap_libraries_path(&abs_path);
    let cbr_path = std::path::Path::new(&physical_path);
    if let (Some(parent), Some(stem)) = (cbr_path.parent(), cbr_path.file_stem()) {
        let cbz_path = parent.join(format!("{}.cbz", stem.to_string_lossy()));
        if cbz_path.exists() {
            return Err(ApiError {
                status: axum::http::StatusCode::CONFLICT,
                message: format!(
                    "CBZ file already exists: {}",
                    unmap_libraries_path(&cbz_path.to_string_lossy())
                ),
            });
        }
    }

    // Create the conversion job
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, book_id, type, status) VALUES ($1, $2, 'cbr_to_cbz', 'pending')",
    )
    .bind(job_id)
    .bind(book_id)
    .execute(&state.pool)
    .await?;

    let job_row = sqlx::query(
        "SELECT id, library_id, book_id, type, status, started_at, finished_at, stats_json, error_opt, created_at, progress_percent, processed_files, total_files FROM index_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(crate::index_jobs::map_row(job_row)))
}

// ─── Metadata editing ─────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct UpdateBookRequest {
    pub title: String,
    pub author: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    pub series: Option<String>,
    pub volume: Option<i32>,
    pub language: Option<String>,
    pub summary: Option<String>,
    pub isbn: Option<String>,
    pub publish_date: Option<String>,
    /// Fields locked from external metadata sync
    #[serde(default)]
    pub locked_fields: Option<serde_json::Value>,
}

/// Update metadata for a specific book
#[utoipa::path(
    patch,
    path = "/books/{id}",
    tag = "books",
    params(("id" = String, Path, description = "Book UUID")),
    request_body = UpdateBookRequest,
    responses(
        (status = 200, body = BookDetails),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Book not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_book(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateBookRequest>,
) -> Result<Json<BookDetails>, ApiError> {
    let title = body.title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::bad_request("title cannot be empty"));
    }
    let author = body.author.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let authors: Vec<String> = body.authors.iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    let series = body.series.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let language = body.language.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);

    let summary = body.summary.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let isbn = body.isbn.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let publish_date = body.publish_date.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let locked_fields = body.locked_fields.clone().unwrap_or(serde_json::json!({}));
    // Resolve series name to series_id
    let series_id: Option<Uuid> = if let Some(ref s) = series {
        // Look up existing series or create one
        let book_row = sqlx::query("SELECT library_id FROM books WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("book not found"))?;
        let lib_id: Uuid = book_row.get("library_id");
        let sid: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO series (id, library_id, name, created_at, updated_at)
            VALUES (gen_random_uuid(), $1, $2, NOW(), NOW())
            ON CONFLICT (library_id, name) DO UPDATE SET updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(lib_id)
        .bind(s)
        .fetch_one(&state.pool)
        .await?;
        Some(sid)
    } else {
        None
    };

    let row = sqlx::query(
        r#"
        UPDATE books
        SET title = $2, author = $3, authors = $4, series_id = $5, volume = $6, language = $7,
            summary = $8, isbn = $9, publish_date = $10, locked_fields = $11, updated_at = NOW()
        WHERE id = $1
        RETURNING id, library_id, kind, title, author, authors, volume, volume_type, language, page_count, thumbnail_path,
                  summary, isbn, publish_date, updated_at,
                  'unread' AS reading_status,
                  NULL::integer AS reading_current_page,
                  NULL::timestamptz AS reading_last_read_at
        "#,
    )
    .bind(id)
    .bind(&title)
    .bind(&author)
    .bind(&authors)
    .bind(series_id)
    .bind(body.volume)
    .bind(&language)
    .bind(&summary)
    .bind(&isbn)
    .bind(&publish_date)
    .bind(&locked_fields)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("book not found"))?;
    let thumbnail_path: Option<String> = row.get("thumbnail_path");

    Ok(Json(BookDetails {
        id: row.get("id"),
        library_id: row.get("library_id"),
        kind: row.get("kind"),
        title: row.get("title"),
        author: row.get("author"),
        authors: row.get::<Vec<String>, _>("authors"),
        series: series.clone(),
        series_id,
        volume: row.get("volume"),
        volume_type: row.get("volume_type"),
        language: row.get("language"),
        page_count: row.get("page_count"),
        thumbnail_url: thumbnail_path.map(|_| format!("/books/{}/thumbnail", id)),
        file_path: None,
        file_format: None,
        file_parse_status: None,
        reading_status: row.get("reading_status"),
        reading_current_page: row.get("reading_current_page"),
        reading_last_read_at: row.get("reading_last_read_at"),
        summary: row.get("summary"),
        isbn: row.get("isbn"),
        publish_date: row.get("publish_date"),
        locked_fields: Some(locked_fields),
        updated_at: row.get("updated_at"),
    }))
}

// ─── Thumbnail ────────────────────────────────────────────────────────────────

use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};

/// Detect content type from thumbnail file extension.
fn detect_thumbnail_content_type(path: &str) -> &'static str {
    if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".png") {
        "image/png"
    } else {
        "image/webp"
    }
}

/// Get book thumbnail image
#[utoipa::path(
    get,
    path = "/books/{id}/thumbnail",
    tag = "books",
    params(
        ("id" = String, Path, description = "Book UUID"),
    ),
    responses(
        (status = 200, description = "WebP thumbnail image", content_type = "image/webp"),
        (status = 304, description = "Not modified (matches If-None-Match)"),
        (status = 404, description = "Book not found or thumbnail not available"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_thumbnail(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
    headers_in: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let row = sqlx::query("SELECT thumbnail_path FROM books WHERE id = $1")
        .bind(book_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let row = row.ok_or_else(|| ApiError::not_found("book not found"))?;
    let thumbnail_path: Option<String> = row.get("thumbnail_path");

    let if_none_match = headers_in
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok());

    // Fast path: if a stored thumbnail exists and its size matches the client's ETag, return 304
    // without reading the file body.
    if let Some(ref path) = thumbnail_path {
        if let Ok(meta) = std::fs::metadata(path) {
            let etag_value = format!("\"{}_{:x}\"", book_id, meta.len());
            if if_none_match == Some(etag_value.as_str()) {
                let mut headers = HeaderMap::new();
                if let Ok(v) = HeaderValue::from_str(&etag_value) {
                    headers.insert(header::ETAG, v);
                }
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, must-revalidate"),
                );
                return Ok((StatusCode::NOT_MODIFIED, headers).into_response());
            }
        }
    }

    let (data, content_type) = if let Some(ref path) = thumbnail_path {
        match std::fs::read(path) {
            Ok(bytes) => {
                let ct = detect_thumbnail_content_type(path);
                (bytes, ct)
            }
            Err(_) => {
                // File missing on disk (e.g. different mount in dev) — fall back to live render
                pages::render_book_page_1(&state, book_id, 300, 80).await?
            }
        }
    } else {
        // No stored thumbnail yet — render page 1 on the fly
        pages::render_book_page_1(&state, book_id, 300, 80).await?
    };

    let etag_value = format!("\"{}_{:x}\"", book_id, data.len());

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, must-revalidate"),
    );
    if let Ok(v) = HeaderValue::from_str(&etag_value) {
        headers.insert(header::ETAG, v);
    }

    Ok((StatusCode::OK, headers, Body::from(data)).into_response())
}

// ─── Delete book ───────────────────────────────────────────────────────────────

/// Delete a book: removes the physical file, the DB record, and queues a library scan.
#[utoipa::path(
    delete,
    path = "/books/{id}",
    tag = "books",
    params(("id" = String, Path, description = "Book UUID")),
    responses(
        (status = 200, description = "Book deleted"),
        (status = 404, description = "Book not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_book(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::responses::OkResponse>, ApiError> {
    // Fetch the book and its file path
    let row = sqlx::query(
        "SELECT b.library_id, b.thumbnail_path, bf.abs_path \
         FROM books b \
         LEFT JOIN book_files bf ON bf.book_id = b.id \
         WHERE b.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("book not found"))?;
    let library_id: Uuid = row.get("library_id");
    let abs_path: Option<String> = row.get("abs_path");
    let thumbnail_path: Option<String> = row.get("thumbnail_path");

    // Delete the physical file
    if let Some(ref path) = abs_path {
        let physical = remap_libraries_path(path);
        match std::fs::remove_file(&physical) {
            Ok(()) => tracing::info!("[BOOKS] Deleted file: {}", physical),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("[BOOKS] File already missing: {}", physical);
            }
            Err(e) => {
                tracing::error!("[BOOKS] Failed to delete file {}: {}", physical, e);
                return Err(ApiError::internal(format!("failed to delete file: {e}")));
            }
        }
    }

    // Delete the thumbnail file
    if let Some(ref path) = thumbnail_path {
        let _ = std::fs::remove_file(path);
    }

    // Delete from DB (book_files cascade via ON DELETE CASCADE)
    sqlx::query("DELETE FROM books WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    // Queue a scan job for the library so the index stays consistent
    let scan_job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(scan_job_id)
    .bind(library_id)
    .execute(&state.pool)
    .await?;

    tracing::info!(
        "[BOOKS] Deleted book {}, scan job {} queued for library {}",
        id, scan_job_id, library_id
    );

    Ok(Json(crate::responses::OkResponse::new()))
}

#[cfg(test)]
#[path = "tests/mod_tests.rs"]
mod tests;
