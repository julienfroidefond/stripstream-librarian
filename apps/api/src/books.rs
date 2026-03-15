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
    #[schema(value_type = Option<String>, example = "cbz")]
    pub format: Option<String>,
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
    pub format: Option<String>,
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

    // Conditions partagées COUNT et DATA — $1=library_id $2=kind $3=format, puis optionnels
    let mut p: usize = 3;
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
             AND ($3::text IS NULL OR b.format = $3)
             {series_cond}
             {rs_cond}"#
    );

    // DATA: mêmes params filtre, puis $N+1=limit $N+2=offset
    let limit_p = p + 1;
    let offset_p = p + 2;
    let data_sql = format!(
        r#"
        SELECT b.id, b.library_id, b.kind, b.format, b.title, b.author, b.series, b.volume, b.language, b.page_count, b.thumbnail_path, b.updated_at,
               COALESCE(brp.status, 'unread') AS reading_status,
               brp.current_page AS reading_current_page,
               brp.last_read_at AS reading_last_read_at
        FROM books b
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
        WHERE ($1::uuid IS NULL OR b.library_id = $1)
          AND ($2::text IS NULL OR b.kind = $2)
          AND ($3::text IS NULL OR b.format = $3)
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
        .bind(query.kind.as_deref())
        .bind(query.format.as_deref());
    let mut data_builder = sqlx::query(&data_sql)
        .bind(query.library_id)
        .bind(query.kind.as_deref())
        .bind(query.format.as_deref());

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
                format: row.get("format"),
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
    #[schema(value_type = String)]
    pub library_id: Uuid,
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
            library_id,
        })
        .collect();

    Ok(Json(SeriesPage {
        items: std::mem::take(&mut items),
        total,
        page,
        limit,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct ListAllSeriesQuery {
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
}

/// List all series across libraries with optional filtering and pagination
#[utoipa::path(
    get,
    path = "/series",
    tag = "books",
    params(
        ("q" = Option<String>, Query, description = "Filter by series name (case-insensitive, partial match)"),
        ("library_id" = Option<String>, Query, description = "Filter by library ID"),
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
pub async fn list_all_series(
    State(state): State<AppState>,
    Query(query): Query<ListAllSeriesQuery>,
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

    let mut p: usize = 0;

    let lib_cond = if query.library_id.is_some() {
        p += 1; format!("WHERE library_id = ${p}")
    } else {
        "WHERE TRUE".to_string()
    };

    let q_cond = if query.q.is_some() {
        p += 1; format!("AND sc.name ILIKE ${p}")
    } else { String::new() };

    let rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND {series_status_expr} = ANY(${p})")
    } else { String::new() };

    let count_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT COALESCE(NULLIF(series, ''), 'unclassified') as name, id
            FROM books {lib_cond}
        ),
        series_counts AS (
            SELECT sb.name,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id
            GROUP BY sb.name
        )
        SELECT COUNT(*) FROM series_counts sc WHERE TRUE {q_cond} {rs_cond}
        "#
    );

    let limit_p = p + 1;
    let offset_p = p + 2;

    let data_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT
                COALESCE(NULLIF(series, ''), 'unclassified') as name,
                id,
                library_id,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        REGEXP_REPLACE(LOWER(title), '[0-9]+', '', 'g'),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) as rn
            FROM books
            {lib_cond}
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
            sb.id as first_book_id,
            sb.library_id
        FROM series_counts sc
        JOIN sorted_books sb ON sb.name = sc.name AND sb.rn = 1
        WHERE TRUE
          {q_cond}
          {rs_cond}
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

    let mut count_builder = sqlx::query(&count_sql);
    let mut data_builder = sqlx::query(&data_sql);

    if let Some(lib_id) = query.library_id {
        count_builder = count_builder.bind(lib_id);
        data_builder = data_builder.bind(lib_id);
    }
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

    let items: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id: row.get("library_id"),
        })
        .collect();

    Ok(Json(SeriesPage {
        items,
        total,
        page,
        limit,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct OngoingQuery {
    #[schema(value_type = Option<i64>, example = 10)]
    pub limit: Option<i64>,
}

/// List ongoing series (partially read, sorted by most recent activity)
#[utoipa::path(
    get,
    path = "/series/ongoing",
    tag = "books",
    params(
        ("limit" = Option<i64>, Query, description = "Max items to return (default 10, max 50)"),
    ),
    responses(
        (status = 200, body = Vec<SeriesItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn ongoing_series(
    State(state): State<AppState>,
    Query(query): Query<OngoingQuery>,
) -> Result<Json<Vec<SeriesItem>>, ApiError> {
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    let rows = sqlx::query(
        r#"
        WITH series_stats AS (
            SELECT
                COALESCE(NULLIF(b.series, ''), 'unclassified') AS name,
                COUNT(*) AS book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') AS books_read_count,
                MAX(brp.last_read_at) AS last_read_at
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
            GROUP BY COALESCE(NULLIF(b.series, ''), 'unclassified')
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        first_books AS (
            SELECT
                COALESCE(NULLIF(series, ''), 'unclassified') AS name,
                id,
                library_id,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        REGEXP_REPLACE(LOWER(title), '[0-9]+', '', 'g'),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) AS rn
            FROM books
        )
        SELECT ss.name, ss.book_count, ss.books_read_count, fb.id AS first_book_id, fb.library_id
        FROM series_stats ss
        JOIN first_books fb ON fb.name = ss.name AND fb.rn = 1
        ORDER BY ss.last_read_at DESC NULLS LAST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            library_id: row.get("library_id"),
        })
        .collect();

    Ok(Json(items))
}

/// List next unread book for each ongoing series (sorted by most recent activity)
#[utoipa::path(
    get,
    path = "/books/ongoing",
    tag = "books",
    params(
        ("limit" = Option<i64>, Query, description = "Max items to return (default 10, max 50)"),
    ),
    responses(
        (status = 200, body = Vec<BookItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn ongoing_books(
    State(state): State<AppState>,
    Query(query): Query<OngoingQuery>,
) -> Result<Json<Vec<BookItem>>, ApiError> {
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    let rows = sqlx::query(
        r#"
        WITH ongoing_series AS (
            SELECT
                COALESCE(NULLIF(b.series, ''), 'unclassified') AS name,
                MAX(brp.last_read_at) AS series_last_read_at
            FROM books b
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
            GROUP BY COALESCE(NULLIF(b.series, ''), 'unclassified')
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        next_books AS (
            SELECT
                b.id, b.library_id, b.kind, b.format, b.title, b.author, b.series, b.volume,
                b.language, b.page_count, b.thumbnail_path, b.updated_at,
                COALESCE(brp.status, 'unread') AS reading_status,
                brp.current_page AS reading_current_page,
                brp.last_read_at AS reading_last_read_at,
                os.series_last_read_at,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(b.series, ''), 'unclassified')
                    ORDER BY b.volume NULLS LAST, b.title
                ) AS rn
            FROM books b
            JOIN ongoing_series os ON COALESCE(NULLIF(b.series, ''), 'unclassified') = os.name
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id
            WHERE COALESCE(brp.status, 'unread') != 'read'
        )
        SELECT id, library_id, kind, format, title, author, series, volume, language, page_count,
               thumbnail_path, updated_at, reading_status, reading_current_page, reading_last_read_at
        FROM next_books
        WHERE rn = 1
        ORDER BY series_last_read_at DESC NULLS LAST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

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
                series: row.get("series"),
                volume: row.get("volume"),
                language: row.get("language"),
                page_count: row.get("page_count"),
                thumbnail_url: thumbnail_path.map(|_| format!("/books/{}/thumbnail", row.get::<Uuid, _>("id"))),
                updated_at: row.get("updated_at"),
                reading_status: row.get("reading_status"),
                reading_current_page: row.get("reading_current_page"),
                reading_last_read_at: row.get("reading_last_read_at"),
            }
        })
        .collect();

    Ok(Json(items))
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

    let (data, content_type) = if let Some(ref path) = thumbnail_path {
        match std::fs::read(path) {
            Ok(bytes) => {
                let ct = detect_thumbnail_content_type(path);
                (bytes, ct)
            }
            Err(_) => {
                // File missing on disk (e.g. different mount in dev) — fall back to live render
                crate::pages::render_book_page_1(&state, book_id, 300, 80).await?
            }
        }
    } else {
        // No stored thumbnail yet — render page 1 on the fly
        crate::pages::render_book_page_1(&state, book_id, 300, 80).await?
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    Ok((StatusCode::OK, headers, Body::from(data)))
}
