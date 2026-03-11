use axum::{extract::{Path, Query, State}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, index_jobs::IndexJobResponse, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct ListBooksQuery {
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub kind: Option<String>,
    #[schema(value_type = Option<String>)]
    pub series: Option<String>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct BookItem {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub library_id: Uuid,
    pub kind: String,
    pub title: String,
    pub author: Option<String>,
    pub series: Option<String>,
    pub volume: Option<i32>,
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
    pub series: Option<String>,
    pub volume: Option<i32>,
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
}

/// List books with optional filtering and pagination
#[utoipa::path(
    get,
    path = "/books",
    tag = "books",
    params(
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
        ("kind" = Option<String>, Query, description = "Filter by book kind (cbz, cbr, pdf)"),
        ("series" = Option<String>, Query, description = "Filter by series name (use 'unclassified' for books without series)"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
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
) -> Result<Json<BooksPage>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    // Parse reading_status CSV → Vec<String>
    let reading_statuses: Option<Vec<String>> = query.reading_status.as_deref().map(|s| {
        s.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect()
    });

    // Conditions partagées COUNT et DATA — $1=library_id $2=kind, puis optionnels
    let mut p: usize = 2;
    let series_cond = match query.series.as_deref() {
        Some("unclassified") => "AND (b.series IS NULL OR b.series = '')".to_string(),
        Some(_) => { p += 1; format!("AND b.series = ${p}") }
        None => String::new(),
    };
    let rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND COALESCE(brp.status, 'unread') = ANY(${p})")
    } else { String::new() };

    let count_sql = format!(
        r#"SELECT COUNT(*) FROM books b
           LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
           WHERE ($1::uuid IS NULL OR b.library_id = $1)
             AND ($2::text IS NULL OR b.kind = $2)
             {series_cond}
             {rs_cond}"#
    );

    // DATA: mêmes params filtre, puis $N+1=limit $N+2=offset
    let limit_p = p + 1;
    let offset_p = p + 2;
    let data_sql = format!(
        r#"
        SELECT b.id, b.library_id, b.kind, b.title, b.author, b.series, b.volume, b.language, b.page_count, b.thumbnail_path, b.updated_at,
               COALESCE(brp.status, 'unread') AS reading_status,
               brp.current_page AS reading_current_page,
               brp.last_read_at AS reading_last_read_at
        FROM books b
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
        WHERE ($1::uuid IS NULL OR b.library_id = $1)
          AND ($2::text IS NULL OR b.kind = $2)
          {series_cond}
          {rs_cond}
        ORDER BY
            REGEXP_REPLACE(LOWER(b.title), '[0-9]+', '', 'g'),
            COALESCE(
                (REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int,
                0
            ),
            b.title ASC
        LIMIT ${limit_p} OFFSET ${offset_p}
        "#
    );

    let mut count_builder = sqlx::query(&count_sql)
        .bind(query.library_id)
        .bind(query.kind.as_deref());
    let mut data_builder = sqlx::query(&data_sql)
        .bind(query.library_id)
        .bind(query.kind.as_deref());

    if let Some(s) = query.series.as_deref() {
        if s != "unclassified" {
            count_builder = count_builder.bind(s);
            data_builder = data_builder.bind(s);
        }
    }
    if let Some(ref statuses) = reading_statuses {
        count_builder = count_builder.bind(statuses.clone());
        data_builder = data_builder.bind(statuses.clone());
    }

    data_builder = data_builder.bind(limit).bind(offset);

    let (count_row, rows) = tokio::try_join!(
        count_builder.fetch_one(&state.pool),
        data_builder.fetch_all(&state.pool),
    )?;
    let total: i64 = count_row.get(0);

    let mut items: Vec<BookItem> = rows
        .iter()
        .map(|row| {
            let thumbnail_path: Option<String> = row.get("thumbnail_path");
            BookItem {
                id: row.get("id"),
                library_id: row.get("library_id"),
                kind: row.get("kind"),
                title: row.get("title"),
                author: row.get("author"),
                series: row.get("series"),
                volume: row.get("volume"),
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
        items: std::mem::take(&mut items),
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
) -> Result<Json<BookDetails>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT b.id, b.library_id, b.kind, b.title, b.author, b.series, b.volume, b.language, b.page_count, b.thumbnail_path,
               bf.abs_path, bf.format, bf.parse_status,
               COALESCE(brp.status, 'unread') AS reading_status,
               brp.current_page AS reading_current_page,
               brp.last_read_at AS reading_last_read_at
        FROM books b
        LEFT JOIN LATERAL (
          SELECT abs_path, format, parse_status
          FROM book_files
          WHERE book_id = b.id
          ORDER BY updated_at DESC
          LIMIT 1
        ) bf ON TRUE
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
        WHERE b.id = $1
        "#,
    )
    .bind(id)
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
        series: row.get("series"),
        volume: row.get("volume"),
        language: row.get("language"),
        page_count: row.get("page_count"),
        thumbnail_url: thumbnail_path.map(|_| format!("/books/{}/thumbnail", id)),
        file_path: row.get("abs_path"),
        file_format: row.get("format"),
        file_parse_status: row.get("parse_status"),
        reading_status: row.get("reading_status"),
        reading_current_page: row.get("reading_current_page"),
        reading_last_read_at: row.get("reading_last_read_at"),
    }))
}

#[derive(Serialize, ToSchema)]
pub struct SeriesItem {
    pub name: String,
    pub book_count: i64,
    pub books_read_count: i64,
    #[schema(value_type = String)]
    pub first_book_id: Uuid,
}

#[derive(Serialize, ToSchema)]
pub struct SeriesPage {
    pub items: Vec<SeriesItem>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct ListSeriesQuery {
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
}

/// List all series in a library with pagination
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series",
    tag = "books",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("q" = Option<String>, Query, description = "Filter by series name (case-insensitive, partial match)"),
        ("reading_status" = Option<String>, Query, description = "Filter by reading status, comma-separated (e.g. 'unread,reading')"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
    ),
    responses(
        (status = 200, body = SeriesPage),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_series(
    State(state): State<AppState>,
    Path(library_id): Path<Uuid>,
    Query(query): Query<ListSeriesQuery>,
) -> Result<Json<SeriesPage>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    let reading_statuses: Option<Vec<String>> = query.reading_status.as_deref().map(|s| {
        s.split(',').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect()
    });

    let series_status_expr = r#"CASE
              WHEN sc.books_read_count = sc.book_count THEN 'read'
              WHEN sc.books_read_count = 0 THEN 'unread'
              ELSE 'reading'
           END"#;

    // Paramètres dynamiques — $1 = library_id fixe, puis optionnels dans l'ordre
    let mut p: usize = 1;

    let q_cond = if query.q.is_some() {
        p += 1; format!("AND sc.name ILIKE ${p}")
    } else { String::new() };

    let count_rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND {series_status_expr} = ANY(${p})")
    } else { String::new() };

    // q_cond et count_rs_cond partagent le même p — le count_sql les réutilise directement
    let count_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT COALESCE(NULLIF(series, ''), 'unclassified') as name, id
            FROM books WHERE library_id = $1
        ),
        series_counts AS (
            SELECT sb.name,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id
            GROUP BY sb.name
        )
        SELECT COUNT(*) FROM series_counts sc WHERE TRUE {q_cond} {count_rs_cond}
        "#
    );

    // DATA: mêmes params dans le même ordre, puis limit/offset à la fin
    let limit_p = p + 1;
    let offset_p = p + 2;

    let data_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT
                COALESCE(NULLIF(series, ''), 'unclassified') as name,
                id,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        REGEXP_REPLACE(LOWER(title), '[0-9]+', '', 'g'),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) as rn
            FROM books
            WHERE library_id = $1
        ),
        series_counts AS (
            SELECT
                sb.name,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id
            GROUP BY sb.name
        )
        SELECT
            sc.name,
            sc.book_count,
            sc.books_read_count,
            sb.id as first_book_id
        FROM series_counts sc
        JOIN sorted_books sb ON sb.name = sc.name AND sb.rn = 1
        WHERE TRUE
          {q_cond}
          {count_rs_cond}
        ORDER BY
            REGEXP_REPLACE(LOWER(sc.name), '[0-9]+', '', 'g'),
            COALESCE(
                (REGEXP_MATCH(LOWER(sc.name), '\d+'))[1]::int,
                0
            ),
            sc.name ASC
        LIMIT ${limit_p} OFFSET ${offset_p}
        "#
    );

    let q_pattern = query.q.as_deref().map(|q| format!("%{}%", q));

    let mut count_builder = sqlx::query(&count_sql).bind(library_id);
    let mut data_builder = sqlx::query(&data_sql).bind(library_id);

    if let Some(ref pat) = q_pattern {
        count_builder = count_builder.bind(pat);
        data_builder = data_builder.bind(pat);
    }
    if let Some(ref statuses) = reading_statuses {
        count_builder = count_builder.bind(statuses.clone());
        data_builder = data_builder.bind(statuses.clone());
    }

    data_builder = data_builder.bind(limit).bind(offset);

    let (count_row, rows) = tokio::try_join!(
        count_builder.fetch_one(&state.pool),
        data_builder.fetch_all(&state.pool),
    )?;
    let total: i64 = count_row.get(0);

    let mut items: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
        })
        .collect();

    Ok(Json(SeriesPage {
        items: std::mem::take(&mut items),
        total,
        page,
        limit,
    }))
}

fn remap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with("/libraries/") {
            return path.replacen("/libraries", &root, 1);
        }
    }
    path.to_string()
}

fn unmap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with(&root) {
            return path.replacen(&root, "/libraries", 1);
        }
    }
    path.to_string()
}

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

use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};

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
        (status = 404, description = "Book not found or thumbnail not available"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_thumbnail(
    State(state): State<AppState>,
    Path(book_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let row = sqlx::query("SELECT thumbnail_path FROM books WHERE id = $1")
        .bind(book_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let row = row.ok_or_else(|| ApiError::not_found("book not found"))?;
    let thumbnail_path: Option<String> = row.get("thumbnail_path");

    let data = if let Some(ref path) = thumbnail_path {
        std::fs::read(path)
            .map_err(|e| ApiError::internal(format!("cannot read thumbnail: {}", e)))?
    } else {
        // Fallback: render page 1 on the fly (same as pages logic)
        crate::pages::render_book_page_1(&state, book_id, 300, 80).await?
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/webp"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok((StatusCode::OK, headers, Body::from(data)))
}
