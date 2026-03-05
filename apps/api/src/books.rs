use axum::{extract::{Path, Query, State}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::ApiError, AppState};

#[derive(Deserialize)]
pub struct ListBooksQuery {
    pub library_id: Option<Uuid>,
    pub kind: Option<String>,
    pub cursor: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct BookItem {
    pub id: Uuid,
    pub library_id: Uuid,
    pub kind: String,
    pub title: String,
    pub author: Option<String>,
    pub series: Option<String>,
    pub volume: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<i32>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct BooksPage {
    pub items: Vec<BookItem>,
    pub next_cursor: Option<Uuid>,
}

#[derive(Serialize)]
pub struct BookDetails {
    pub id: Uuid,
    pub library_id: Uuid,
    pub kind: String,
    pub title: String,
    pub author: Option<String>,
    pub series: Option<String>,
    pub volume: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<i32>,
    pub file_path: Option<String>,
    pub file_format: Option<String>,
    pub file_parse_status: Option<String>,
}

pub async fn list_books(
    State(state): State<AppState>,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<BooksPage>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let rows = sqlx::query(
        r#"
        SELECT id, library_id, kind, title, author, series, volume, language, page_count, updated_at
        FROM books
        WHERE ($1::uuid IS NULL OR library_id = $1)
          AND ($2::text IS NULL OR kind = $2)
          AND ($3::uuid IS NULL OR id > $3)
        ORDER BY id ASC
        LIMIT $4
        "#,
    )
    .bind(query.library_id)
    .bind(query.kind.as_deref())
    .bind(query.cursor)
    .bind(limit + 1)
    .fetch_all(&state.pool)
    .await?;

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
