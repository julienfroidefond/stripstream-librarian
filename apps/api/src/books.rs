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
    /// Sort order: "title" (default) or "latest" (most recently added first)
    #[schema(value_type = Option<String>, example = "latest")]
    pub sort: Option<String>,
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
    pub summary: Option<String>,
    pub isbn: Option<String>,
    pub publish_date: Option<String>,
    /// Fields locked from external metadata sync
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_fields: Option<serde_json::Value>,
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
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default) or 'latest' (most recently added first)"),
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

    let order_clause = if query.sort.as_deref() == Some("latest") {
        "b.updated_at DESC".to_string()
    } else {
        "b.volume NULLS LAST, REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''), COALESCE((REGEXP_MATCH(LOWER(b.title), '\\d+'))[1]::int, 0), b.title ASC".to_string()
    };

    // DATA: mêmes params filtre, puis $N+1=limit $N+2=offset
    let limit_p = p + 1;
    let offset_p = p + 2;
    let data_sql = format!(
        r#"
        SELECT b.id, b.library_id, b.kind, b.format, b.title, b.author, b.authors, b.series, b.volume, b.language, b.page_count, b.thumbnail_path, b.updated_at,
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
                authors: row.get::<Vec<String>, _>("authors"),
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
        SELECT b.id, b.library_id, b.kind, b.title, b.author, b.authors, b.series, b.volume, b.language, b.page_count, b.thumbnail_path, b.locked_fields, b.summary, b.isbn, b.publish_date,
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
        authors: row.get::<Vec<String>, _>("authors"),
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
        summary: row.get("summary"),
        isbn: row.get("isbn"),
        publish_date: row.get("publish_date"),
        locked_fields: Some(row.get::<serde_json::Value, _>("locked_fields")),
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
    pub series_status: Option<String>,
    pub missing_count: Option<i64>,
    pub metadata_provider: Option<String>,
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
    /// Filter by series status (e.g. "ongoing", "ended")
    #[schema(value_type = Option<String>, example = "ongoing")]
    pub series_status: Option<String>,
    /// Filter series with missing books: "true" to show only series with missing books
    #[schema(value_type = Option<String>, example = "true")]
    pub has_missing: Option<String>,
    /// Filter by metadata provider: a provider name (e.g. "google_books"), "linked" (any provider), or "unlinked" (no provider)
    #[schema(value_type = Option<String>, example = "google_books")]
    pub metadata_provider: Option<String>,
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
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
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

    let has_missing = query.has_missing.as_deref() == Some("true");

    // Paramètres dynamiques — $1 = library_id fixe, puis optionnels dans l'ordre
    let mut p: usize = 1;

    let q_cond = if query.q.is_some() {
        p += 1; format!("AND sc.name ILIKE ${p}")
    } else { String::new() };

    let count_rs_cond = if reading_statuses.is_some() {
        p += 1; format!("AND {series_status_expr} = ANY(${p})")
    } else { String::new() };

    let ss_cond = if query.series_status.is_some() {
        p += 1; format!("AND LOWER(sm.status) = ${p}")
    } else { String::new() };

    let missing_cond = if has_missing {
        "AND mc.missing_count > 0".to_string()
    } else { String::new() };

    let metadata_provider_cond = match query.metadata_provider.as_deref() {
        Some("unlinked") => "AND ml.provider IS NULL".to_string(),
        Some("linked") => "AND ml.provider IS NOT NULL".to_string(),
        Some(_) => { p += 1; format!("AND ml.provider = ${p}") },
        None => String::new(),
    };

    let missing_cte = r#"
        missing_counts AS (
            SELECT eml.series_name,
                COUNT(ebm.id) FILTER (WHERE ebm.book_id IS NULL) as missing_count
            FROM external_metadata_links eml
            JOIN external_book_metadata ebm ON ebm.link_id = eml.id
            WHERE eml.library_id = $1 AND eml.status = 'approved'
            GROUP BY eml.series_name
        )
        "#.to_string();

    let metadata_links_cte = r#"
        metadata_links AS (
            SELECT DISTINCT ON (eml.series_name, eml.library_id)
                eml.series_name, eml.library_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_name, eml.library_id, eml.created_at DESC
        )
    "#;

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
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT COUNT(*) FROM series_counts sc
        LEFT JOIN series_metadata sm ON sm.library_id = $1 AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = $1
        WHERE TRUE {q_cond} {count_rs_cond} {ss_cond} {missing_cond} {metadata_provider_cond}
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
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(title), '[0-9].*$', ''),
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
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT
            sc.name,
            sc.book_count,
            sc.books_read_count,
            sb.id as first_book_id,
            sm.status as series_status,
            mc.missing_count,
            ml.provider as metadata_provider
        FROM series_counts sc
        JOIN sorted_books sb ON sb.name = sc.name AND sb.rn = 1
        LEFT JOIN series_metadata sm ON sm.library_id = $1 AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = $1
        WHERE TRUE
          {q_cond}
          {count_rs_cond}
          {ss_cond}
          {missing_cond}
          {metadata_provider_cond}
        ORDER BY
            REGEXP_REPLACE(LOWER(sc.name), '[0-9].*$', ''),
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
    if let Some(ref ss) = query.series_status {
        count_builder = count_builder.bind(ss);
        data_builder = data_builder.bind(ss);
    }
    if let Some(ref mp) = query.metadata_provider {
        if mp != "linked" && mp != "unlinked" {
            count_builder = count_builder.bind(mp);
            data_builder = data_builder.bind(mp);
        }
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
            library_id,
            series_status: row.get("series_status"),
            missing_count: row.get("missing_count"),
            metadata_provider: row.get("metadata_provider"),
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
pub struct ListAllSeriesQuery {
    #[schema(value_type = Option<String>, example = "dragon")]
    pub q: Option<String>,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>, example = "unread,reading")]
    pub reading_status: Option<String>,
    /// Filter by series status (e.g. "ongoing", "ended")
    #[schema(value_type = Option<String>, example = "ongoing")]
    pub series_status: Option<String>,
    /// Filter series with missing books: "true" to show only series with missing books
    #[schema(value_type = Option<String>, example = "true")]
    pub has_missing: Option<String>,
    /// Filter by metadata provider: a provider name (e.g. "google_books"), "linked" (any provider), or "unlinked" (no provider)
    #[schema(value_type = Option<String>, example = "google_books")]
    pub metadata_provider: Option<String>,
    #[schema(value_type = Option<i64>, example = 1)]
    pub page: Option<i64>,
    #[schema(value_type = Option<i64>, example = 50)]
    pub limit: Option<i64>,
    /// Sort order: "title" (default) or "latest" (most recently added first)
    #[schema(value_type = Option<String>, example = "latest")]
    pub sort: Option<String>,
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
        ("metadata_provider" = Option<String>, Query, description = "Filter by metadata provider: a provider name (e.g. 'google_books'), 'linked' (any provider), or 'unlinked' (no provider)"),
        ("page" = Option<i64>, Query, description = "Page number (1-indexed, default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (max 200, default 50)"),
        ("sort" = Option<String>, Query, description = "Sort order: 'title' (default) or 'latest' (most recently added first)"),
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

    let has_missing = query.has_missing.as_deref() == Some("true");

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

    let ss_cond = if query.series_status.is_some() {
        p += 1; format!("AND LOWER(sm.status) = ${p}")
    } else { String::new() };

    let missing_cond = if has_missing {
        "AND mc.missing_count > 0".to_string()
    } else { String::new() };

    let metadata_provider_cond = match query.metadata_provider.as_deref() {
        Some("unlinked") => "AND ml.provider IS NULL".to_string(),
        Some("linked") => "AND ml.provider IS NOT NULL".to_string(),
        Some(_) => { p += 1; format!("AND ml.provider = ${p}") },
        None => String::new(),
    };

    // Missing counts CTE — needs library_id filter when filtering by library
    let missing_cte = if query.library_id.is_some() {
        r#"
            missing_counts AS (
                SELECT eml.series_name, eml.library_id,
                    COUNT(ebm.id) FILTER (WHERE ebm.book_id IS NULL) as missing_count
                FROM external_metadata_links eml
                JOIN external_book_metadata ebm ON ebm.link_id = eml.id
                WHERE eml.library_id = $1 AND eml.status = 'approved'
                GROUP BY eml.series_name, eml.library_id
            )
            "#.to_string()
    } else {
        r#"
        missing_counts AS (
            SELECT eml.series_name, eml.library_id,
                COUNT(ebm.id) FILTER (WHERE ebm.book_id IS NULL) as missing_count
            FROM external_metadata_links eml
            JOIN external_book_metadata ebm ON ebm.link_id = eml.id
            WHERE eml.status = 'approved'
            GROUP BY eml.series_name, eml.library_id
        )
        "#.to_string()
    };

    let metadata_links_cte = r#"
        metadata_links AS (
            SELECT DISTINCT ON (eml.series_name, eml.library_id)
                eml.series_name, eml.library_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.status = 'approved'
            ORDER BY eml.series_name, eml.library_id, eml.created_at DESC
        )
    "#;

    let count_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT COALESCE(NULLIF(series, ''), 'unclassified') as name, id, library_id
            FROM books {lib_cond}
        ),
        series_counts AS (
            SELECT sb.name, sb.library_id,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id
            GROUP BY sb.name, sb.library_id
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT COUNT(*) FROM series_counts sc
        LEFT JOIN series_metadata sm ON sm.library_id = sc.library_id AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name AND mc.library_id = sc.library_id
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = sc.library_id
        WHERE TRUE {q_cond} {rs_cond} {ss_cond} {missing_cond} {metadata_provider_cond}
        "#
    );

    let series_order_clause = if query.sort.as_deref() == Some("latest") {
        "sc.latest_updated_at DESC".to_string()
    } else {
        "REGEXP_REPLACE(LOWER(sc.name), '[0-9].*$', ''), COALESCE((REGEXP_MATCH(LOWER(sc.name), '\\d+'))[1]::int, 0), sc.name ASC".to_string()
    };

    let limit_p = p + 1;
    let offset_p = p + 2;

    let data_sql = format!(
        r#"
        WITH sorted_books AS (
            SELECT
                COALESCE(NULLIF(series, ''), 'unclassified') as name,
                id,
                library_id,
                updated_at,
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified')
                    ORDER BY
                        volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(title), '\d+'))[1]::int, 0),
                        title ASC
                ) as rn
            FROM books
            {lib_cond}
        ),
        series_counts AS (
            SELECT
                sb.name,
                sb.library_id,
                COUNT(*) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count,
                MAX(sb.updated_at) as latest_updated_at
            FROM sorted_books sb
            LEFT JOIN book_reading_progress brp ON brp.book_id = sb.id
            GROUP BY sb.name, sb.library_id
        ),
        {missing_cte},
        {metadata_links_cte}
        SELECT
            sc.name,
            sc.book_count,
            sc.books_read_count,
            sb.id as first_book_id,
            sb.library_id,
            sm.status as series_status,
            mc.missing_count,
            ml.provider as metadata_provider
        FROM series_counts sc
        JOIN sorted_books sb ON sb.name = sc.name AND sb.rn = 1
        LEFT JOIN series_metadata sm ON sm.library_id = sc.library_id AND sm.name = sc.name
        LEFT JOIN missing_counts mc ON mc.series_name = sc.name AND mc.library_id = sc.library_id
        LEFT JOIN metadata_links ml ON ml.series_name = sc.name AND ml.library_id = sc.library_id
        WHERE TRUE
          {q_cond}
          {rs_cond}
          {ss_cond}
          {missing_cond}
          {metadata_provider_cond}
        ORDER BY {series_order_clause}
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
    if let Some(ref ss) = query.series_status {
        count_builder = count_builder.bind(ss);
        data_builder = data_builder.bind(ss);
    }
    if let Some(ref mp) = query.metadata_provider {
        if mp != "linked" && mp != "unlinked" {
            count_builder = count_builder.bind(mp);
            data_builder = data_builder.bind(mp);
        }
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
            series_status: row.get("series_status"),
            missing_count: row.get("missing_count"),
            metadata_provider: row.get("metadata_provider"),
        })
        .collect();

    Ok(Json(SeriesPage {
        items,
        total,
        page,
        limit,
    }))
}

/// List all distinct series status values present in the database
#[utoipa::path(
    get,
    path = "/series/statuses",
    tag = "books",
    responses(
        (status = 200, body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn series_statuses(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"SELECT DISTINCT s FROM (
            SELECT LOWER(status) AS s FROM series_metadata WHERE status IS NOT NULL
            UNION
            SELECT mapped_status AS s FROM status_mappings
        ) t ORDER BY s"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// List distinct raw provider statuses from external metadata links
#[utoipa::path(
    get,
    path = "/series/provider-statuses",
    tag = "books",
    responses(
        (status = 200, body = Vec<String>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn provider_statuses(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"SELECT DISTINCT lower(metadata_json->>'status') AS s
           FROM external_metadata_links
           WHERE metadata_json->>'status' IS NOT NULL
             AND metadata_json->>'status' != ''
           ORDER BY s"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
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
                        volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(title), '[0-9].*$', ''),
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
            series_status: None,
            missing_count: None,
            metadata_provider: None,
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
                b.id, b.library_id, b.kind, b.format, b.title, b.author, b.authors, b.series, b.volume,
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
        SELECT id, library_id, kind, format, title, author, authors, series, volume, language, page_count,
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
                authors: row.get::<Vec<String>, _>("authors"),
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
    let row = sqlx::query(
        r#"
        UPDATE books
        SET title = $2, author = $3, authors = $4, series = $5, volume = $6, language = $7,
            summary = $8, isbn = $9, publish_date = $10, locked_fields = $11, updated_at = NOW()
        WHERE id = $1
        RETURNING id, library_id, kind, title, author, authors, series, volume, language, page_count, thumbnail_path,
                  summary, isbn, publish_date,
                  COALESCE((SELECT status FROM book_reading_progress WHERE book_id = $1), 'unread') AS reading_status,
                  (SELECT current_page FROM book_reading_progress WHERE book_id = $1) AS reading_current_page,
                  (SELECT last_read_at FROM book_reading_progress WHERE book_id = $1) AS reading_last_read_at
        "#,
    )
    .bind(id)
    .bind(&title)
    .bind(&author)
    .bind(&authors)
    .bind(&series)
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
        series: row.get("series"),
        volume: row.get("volume"),
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
    }))
}

#[derive(Serialize, ToSchema)]
pub struct SeriesMetadata {
    /// Authors of the series (series-level metadata, distinct from per-book author field)
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    /// Series status: "ongoing", "ended", "hiatus", "cancelled", or null
    pub status: Option<String>,
    /// Convenience: author from first book (for pre-filling the per-book apply section)
    pub book_author: Option<String>,
    pub book_language: Option<String>,
    /// Fields locked from external metadata sync, e.g. {"authors": true, "description": true}
    pub locked_fields: serde_json::Value,
}

/// Get metadata for a specific series
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series/{name}/metadata",
    tag = "books",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("name" = String, Path, description = "Series name"),
    ),
    responses(
        (status = 200, body = SeriesMetadata),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_metadata(
    State(state): State<AppState>,
    Path((library_id, name)): Path<(Uuid, String)>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    // author/language from first book of series
    let books_row = if name == "unclassified" {
        sqlx::query("SELECT author, language FROM books WHERE library_id = $1 AND (series IS NULL OR series = '') LIMIT 1")
            .bind(library_id)
            .fetch_optional(&state.pool)
            .await?
    } else {
        sqlx::query("SELECT author, language FROM books WHERE library_id = $1 AND series = $2 LIMIT 1")
            .bind(library_id)
            .bind(&name)
            .fetch_optional(&state.pool)
            .await?
    };

    let meta_row = sqlx::query(
        "SELECT authors, description, publishers, start_year, total_volumes, status, locked_fields FROM series_metadata WHERE library_id = $1 AND name = $2"
    )
    .bind(library_id)
    .bind(&name)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(SeriesMetadata {
        authors: meta_row.as_ref().map(|r| r.get::<Vec<String>, _>("authors")).unwrap_or_default(),
        description: meta_row.as_ref().and_then(|r| r.get("description")),
        publishers: meta_row.as_ref().map(|r| r.get::<Vec<String>, _>("publishers")).unwrap_or_default(),
        start_year: meta_row.as_ref().and_then(|r| r.get("start_year")),
        total_volumes: meta_row.as_ref().and_then(|r| r.get("total_volumes")),
        status: meta_row.as_ref().and_then(|r| r.get("status")),
        book_author: books_row.as_ref().and_then(|r| r.get("author")),
        book_language: books_row.as_ref().and_then(|r| r.get("language")),
        locked_fields: meta_row.as_ref().map(|r| r.get::<serde_json::Value, _>("locked_fields")).unwrap_or(serde_json::json!({})),
    }))
}

/// `author` and `language` are wrapped in an extra Option so we can distinguish
/// "absent from JSON" (keep books unchanged) from "present as null" (clear the field).
#[derive(Deserialize, ToSchema)]
pub struct UpdateSeriesRequest {
    pub new_name: String,
    /// Series-level authors list (stored in series_metadata)
    #[serde(default)]
    pub authors: Vec<String>,
    /// Per-book author propagation: absent = keep books unchanged, present = overwrite all books
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Option<String>>,
    /// Per-book language propagation: absent = keep books unchanged, present = overwrite all books
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<Option<String>>,
    pub description: Option<String>,
    #[serde(default)]
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    /// Series status: "ongoing", "ended", "hiatus", "cancelled", or null
    pub status: Option<String>,
    /// Fields locked from external metadata sync
    #[serde(default)]
    pub locked_fields: Option<serde_json::Value>,
}

#[derive(Serialize, ToSchema)]
pub struct UpdateSeriesResponse {
    pub updated: u64,
}

/// Update metadata for all books in a series
#[utoipa::path(
    patch,
    path = "/libraries/{library_id}/series/{name}",
    tag = "books",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("name" = String, Path, description = "Series name (use 'unclassified' for books without series)"),
    ),
    request_body = UpdateSeriesRequest,
    responses(
        (status = 200, body = UpdateSeriesResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_series(
    State(state): State<AppState>,
    Path((library_id, name)): Path<(Uuid, String)>,
    Json(body): Json<UpdateSeriesRequest>,
) -> Result<Json<UpdateSeriesResponse>, ApiError> {
    let new_name = body.new_name.trim().to_string();
    if new_name.is_empty() {
        return Err(ApiError::bad_request("series name cannot be empty"));
    }
    // author/language: None = absent (keep books unchanged), Some(v) = apply to all books
    let apply_author = body.author.is_some();
    let author_value = body.author.flatten().as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let apply_language = body.language.is_some();
    let language_value = body.language.flatten().as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let description = body.description.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let publishers: Vec<String> = body.publishers.iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    let new_series_value: Option<String> = if new_name == "unclassified" { None } else { Some(new_name.clone()) };

    // 1. Update books: always update series name; author/language only if opted-in
    // $1=library_id, $2=new_series_value, $3=apply_author, $4=author_value,
    // $5=apply_language, $6=language_value, [$7=old_name]
    let result = if name == "unclassified" {
        sqlx::query(
            "UPDATE books \
             SET series = $2, \
                 author = CASE WHEN $3 THEN $4 ELSE author END, \
                 language = CASE WHEN $5 THEN $6 ELSE language END, \
                 updated_at = NOW() \
             WHERE library_id = $1 AND (series IS NULL OR series = '')"
        )
        .bind(library_id)
        .bind(&new_series_value)
        .bind(apply_author)
        .bind(&author_value)
        .bind(apply_language)
        .bind(&language_value)
        .execute(&state.pool)
        .await?
    } else {
        sqlx::query(
            "UPDATE books \
             SET series = $2, \
                 author = CASE WHEN $3 THEN $4 ELSE author END, \
                 language = CASE WHEN $5 THEN $6 ELSE language END, \
                 updated_at = NOW() \
             WHERE library_id = $1 AND series = $7"
        )
        .bind(library_id)
        .bind(&new_series_value)
        .bind(apply_author)
        .bind(&author_value)
        .bind(apply_language)
        .bind(&language_value)
        .bind(&name)
        .execute(&state.pool)
        .await?
    };

    // 2. Upsert series_metadata (keyed by new_name)
    let meta_name = new_series_value.as_deref().unwrap_or("unclassified");
    let authors: Vec<String> = body.authors.iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    let locked_fields = body.locked_fields.clone().unwrap_or(serde_json::json!({}));
    sqlx::query(
        r#"
        INSERT INTO series_metadata (library_id, name, authors, description, publishers, start_year, total_volumes, status, locked_fields, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
        ON CONFLICT (library_id, name) DO UPDATE
          SET authors     = EXCLUDED.authors,
              description = EXCLUDED.description,
              publishers  = EXCLUDED.publishers,
              start_year  = EXCLUDED.start_year,
              total_volumes = EXCLUDED.total_volumes,
              status      = EXCLUDED.status,
              locked_fields = EXCLUDED.locked_fields,
              updated_at  = NOW()
        "#
    )
    .bind(library_id)
    .bind(meta_name)
    .bind(&authors)
    .bind(&description)
    .bind(&publishers)
    .bind(body.start_year)
    .bind(body.total_volumes)
    .bind(&body.status)
    .bind(&locked_fields)
    .execute(&state.pool)
    .await?;

    // 3. If renamed, move series_metadata from old name to new name
    if name != "unclassified" && new_name != name {
        sqlx::query(
            "DELETE FROM series_metadata WHERE library_id = $1 AND name = $2"
        )
        .bind(library_id)
        .bind(&name)
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(UpdateSeriesResponse { updated: result.rows_affected() }))
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
