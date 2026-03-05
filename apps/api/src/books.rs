use axum::{extract::{Path, Query, State}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, AppState};

#[derive(Deserialize, ToSchema)]
pub struct ListBooksQuery {
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub kind: Option<String>,
    #[schema(value_type = Option<String>)]
    pub series: Option<String>,
    #[schema(value_type = Option<String>)]
    pub cursor: Option<Uuid>,
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
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct BooksPage {
    pub items: Vec<BookItem>,
    #[schema(value_type = Option<String>)]
    pub next_cursor: Option<Uuid>,
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
    pub file_path: Option<String>,
    pub file_format: Option<String>,
    pub file_parse_status: Option<String>,
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
        ("cursor" = Option<String>, Query, description = "Cursor for pagination"),
        ("limit" = Option<i64>, Query, description = "Max items to return (max 200)"),
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
    
    // Build series filter condition
    let series_condition = match query.series.as_deref() {
        Some("unclassified") => "AND (series IS NULL OR series = '')",
        Some(_series_name) => "AND series = $5",
        None => "",
    };
    
    let sql = format!(
        r#"
        SELECT id, library_id, kind, title, author, series, volume, language, page_count, updated_at
        FROM books
        WHERE ($1::uuid IS NULL OR library_id = $1)
          AND ($2::text IS NULL OR kind = $2)
          AND ($3::uuid IS NULL OR id > $3)
          {}
        ORDER BY 
            -- Extract text part before numbers (case insensitive)
            REGEXP_REPLACE(LOWER(title), '[0-9]+', '', 'g'),
            -- Extract first number group and convert to integer for numeric sort
            COALESCE(
                NULLIF(REGEXP_REPLACE(LOWER(title), '^[^0-9]*', '', 'g'), '')::int, 
                0
            ),
            -- Then by full title as fallback
            title ASC
        LIMIT $4
        "#,
        series_condition
    );
    
    let mut query_builder = sqlx::query(&sql)
        .bind(query.library_id)
        .bind(query.kind.as_deref())
        .bind(query.cursor)
        .bind(limit + 1);
    
    // Bind series parameter if it's not unclassified
    if let Some(series) = query.series.as_deref() {
        if series != "unclassified" {
            query_builder = query_builder.bind(series);
        }
    }
    
    let rows = query_builder.fetch_all(&state.pool).await?;

    let mut items: Vec<BookItem> = rows
        .iter()
        .take(limit as usize)
        .map(|row| BookItem {
            id: row.get("id"),
            library_id: row.get("library_id"),
            kind: row.get("kind"),
            title: row.get("title"),
            author: row.get("author"),
            series: row.get("series"),
            volume: row.get("volume"),
            language: row.get("language"),
            page_count: row.get("page_count"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    let next_cursor = if rows.len() > limit as usize {
        items.last().map(|b| b.id)
    } else {
        None
    };

    Ok(Json(BooksPage {
        items: std::mem::take(&mut items),
        next_cursor,
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
        SELECT b.id, b.library_id, b.kind, b.title, b.author, b.series, b.volume, b.language, b.page_count,
               bf.abs_path, bf.format, bf.parse_status
        FROM books b
        LEFT JOIN LATERAL (
          SELECT abs_path, format, parse_status
          FROM book_files
          WHERE book_id = b.id
          ORDER BY updated_at DESC
          LIMIT 1
        ) bf ON TRUE
        WHERE b.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| ApiError::not_found("book not found"))?;
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
        file_path: row.get("abs_path"),
        file_format: row.get("format"),
        file_parse_status: row.get("parse_status"),
    }))
}

#[derive(Serialize, ToSchema)]
pub struct SeriesItem {
    pub name: String,
    pub book_count: i64,
    #[schema(value_type = String)]
    pub first_book_id: Uuid,
}

/// List all series in a library
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series",
    tag = "books",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
    ),
    responses(
        (status = 200, body = Vec<SeriesItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_series(
    State(state): State<AppState>,
    Path(library_id): Path<Uuid>,
) -> Result<Json<Vec<SeriesItem>>, ApiError> {
    let rows = sqlx::query(
        r#"
        WITH sorted_books AS (
            SELECT 
                COALESCE(NULLIF(series, ''), 'unclassified') as name,
                id,
                -- Natural sort order for books within series
                ROW_NUMBER() OVER (
                    PARTITION BY COALESCE(NULLIF(series, ''), 'unclassified') 
                    ORDER BY 
                        REGEXP_REPLACE(LOWER(title), '[0-9]+', '', 'g'),
                        COALESCE(NULLIF(REGEXP_REPLACE(LOWER(title), '^[^0-9]*', '', 'g'), '')::int, 0),
                        title ASC
                ) as rn
            FROM books
            WHERE library_id = $1
        ),
        series_counts AS (
            SELECT 
                name,
                COUNT(*) as book_count
            FROM sorted_books
            GROUP BY name
        )
        SELECT 
            sc.name,
            sc.book_count,
            sb.id as first_book_id
        FROM series_counts sc
        JOIN sorted_books sb ON sb.name = sc.name AND sb.rn = 1
        ORDER BY 
            -- Natural sort: extract text part before numbers
            REGEXP_REPLACE(LOWER(sc.name), '[0-9]+', '', 'g'),
            -- Extract first number group and convert to integer
            COALESCE(
                NULLIF(REGEXP_REPLACE(LOWER(sc.name), '^[^0-9]*', '', 'g'), '')::int, 
                0
            ),
            sc.name ASC
        "#,
    )
    .bind(library_id)
    .fetch_all(&state.pool)
    .await?;

    let series: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            book_count: row.get("book_count"),
            first_book_id: row.get("first_book_id"),
        })
        .collect();

    Ok(Json(series))
}
