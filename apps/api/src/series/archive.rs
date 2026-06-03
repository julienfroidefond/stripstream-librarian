use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ArchivedSeriesReadingProgress {
    pub user_name: String,
    pub books_read: i64,
    pub books_reading: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ArchivedSeriesItem {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub publishers: Vec<String>,
    pub genres: Vec<String>,
    pub cover_url: Option<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    pub status: Option<String>,
    pub book_count: i64,
    pub archived_at: DateTime<Utc>,
    pub reading_progress: Vec<ArchivedSeriesReadingProgress>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ArchivedBookReadingProgress {
    pub user_name: String,
    pub status: String,
    pub current_page: Option<i32>,
    pub last_read_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
pub struct ArchivedBookItem {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub title: String,
    pub kind: String,
    pub format: Option<String>,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub page_count: Option<i32>,
    pub thumbnail_path: Option<String>,
    pub author: Option<String>,
    pub authors: Vec<String>,
    pub language: Option<String>,
    pub isbn: Option<String>,
    pub publish_date: Option<String>,
    pub abs_path: Option<String>,
    pub archived_at: DateTime<Utc>,
    pub reading_progress: Vec<ArchivedBookReadingProgress>,
}

#[derive(Serialize, ToSchema)]
pub struct ArchivedSeriesDetail {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    #[schema(value_type = Option<String>)]
    pub library_id: Option<Uuid>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub publishers: Vec<String>,
    pub genres: Vec<String>,
    pub cover_url: Option<String>,
    pub total_volumes: Option<i32>,
    pub status: Option<String>,
    pub start_year: Option<i32>,
    pub archived_at: DateTime<Utc>,
    pub books: Vec<ArchivedBookItem>,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// List all archived series
#[utoipa::path(
    get,
    path = "/admin/series/archived",
    tag = "series",
    responses(
        (status = 200, body = Vec<ArchivedSeriesItem>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_archived_series(
    State(state): State<AppState>,
) -> Result<Json<Vec<ArchivedSeriesItem>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            aseries.id,
            aseries.name,
            aseries.library_id,
            aseries.description,
            COALESCE(aseries.authors, ARRAY[]::text[]) AS authors,
            COALESCE(aseries.publishers, ARRAY[]::text[]) AS publishers,
            COALESCE(aseries.genres, ARRAY[]::text[]) AS genres,
            aseries.cover_url,
            aseries.start_year,
            aseries.total_volumes,
            aseries.status,
            aseries.archived_at,
            COUNT(ab.id) AS book_count,
            (
                SELECT COALESCE(
                    json_agg(
                        json_build_object(
                            'user_name', u.username,
                            'books_read', rp.books_read,
                            'books_reading', rp.books_reading
                        ) ORDER BY u.username
                    ),
                    '[]'::json
                )
                FROM (
                    SELECT abrp.user_id,
                           SUM(CASE WHEN abrp.status = 'read' THEN 1 ELSE 0 END) AS books_read,
                           SUM(CASE WHEN abrp.status = 'reading' THEN 1 ELSE 0 END) AS books_reading
                    FROM archived_book_reading_progress abrp
                    JOIN archived_books ab2 ON ab2.id = abrp.archived_book_id
                    WHERE ab2.series_id = aseries.id
                    GROUP BY abrp.user_id
                ) rp
                JOIN users u ON u.id = rp.user_id
            ) AS reading_progress
        FROM archived_series aseries
        LEFT JOIN archived_books ab ON ab.series_id = aseries.id
        GROUP BY aseries.id
        ORDER BY aseries.archived_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let progress_json: serde_json::Value = row.try_get("reading_progress").unwrap_or(serde_json::Value::Array(vec![]));
            let reading_progress: Vec<ArchivedSeriesReadingProgress> =
                serde_json::from_value(progress_json).unwrap_or_default();
            ArchivedSeriesItem {
                id: row.get("id"),
                name: row.get("name"),
                library_id: row.get("library_id"),
                description: row.get("description"),
                authors: row.try_get::<Vec<String>, _>("authors").unwrap_or_default(),
                publishers: row.try_get::<Vec<String>, _>("publishers").unwrap_or_default(),
                genres: row.try_get::<Vec<String>, _>("genres").unwrap_or_default(),
                cover_url: row.get("cover_url"),
                start_year: row.get("start_year"),
                total_volumes: row.get("total_volumes"),
                status: row.get("status"),
                book_count: row.get("book_count"),
                archived_at: row.get("archived_at"),
                reading_progress,
            }
        })
        .collect();

    Ok(Json(items))
}

/// Get details of an archived series including its books
#[utoipa::path(
    get,
    path = "/admin/series/archived/{id}",
    tag = "series",
    params(("id" = String, Path, description = "Archived series UUID")),
    responses(
        (status = 200, body = ArchivedSeriesDetail),
        (status = 404, description = "Not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_archived_series(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ArchivedSeriesDetail>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT id, name, library_id, description,
               COALESCE(authors, ARRAY[]::text[]) AS authors,
               COALESCE(publishers, ARRAY[]::text[]) AS publishers,
               COALESCE(genres, ARRAY[]::text[]) AS genres,
               cover_url, total_volumes, status, start_year, archived_at
        FROM archived_series WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("archived series not found"))?;

    let book_rows = sqlx::query(
        r#"
        SELECT ab.id, ab.title, ab.kind, ab.format, ab.volume, ab.volume_type,
               ab.page_count, ab.thumbnail_path, ab.author,
               COALESCE(ab.authors, ARRAY[]::text[]) AS authors,
               ab.language, ab.isbn, ab.publish_date, ab.archived_at,
               MAX(abf.abs_path) AS abs_path,
               COALESCE(
                   json_agg(
                       json_build_object(
                           'user_name', u.username,
                           'status', abrp.status,
                           'current_page', abrp.current_page,
                           'last_read_at', abrp.last_read_at
                       ) ORDER BY u.username
                   ) FILTER (WHERE abrp.archived_book_id IS NOT NULL),
                   '[]'::json
               ) AS reading_progress
        FROM archived_books ab
        LEFT JOIN archived_book_files abf ON abf.archived_book_id = ab.id
        LEFT JOIN archived_book_reading_progress abrp ON abrp.archived_book_id = ab.id
        LEFT JOIN users u ON u.id = abrp.user_id
        WHERE ab.series_id = $1
        GROUP BY ab.id
        ORDER BY
            CASE WHEN ab.volume_type = 'regular' THEN 0 ELSE 1 END,
            ab.volume NULLS LAST,
            ab.title ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let books = book_rows
        .into_iter()
        .map(|r| {
            let progress_json: serde_json::Value = r.try_get("reading_progress").unwrap_or(serde_json::Value::Array(vec![]));
            let reading_progress: Vec<ArchivedBookReadingProgress> =
                serde_json::from_value(progress_json).unwrap_or_default();
            ArchivedBookItem {
                id: r.get("id"),
                title: r.get("title"),
                kind: r.get("kind"),
                format: r.get("format"),
                volume: r.get("volume"),
                volume_type: r.get("volume_type"),
                page_count: r.get("page_count"),
                thumbnail_path: r.get("thumbnail_path"),
                author: r.get("author"),
                authors: r.try_get::<Vec<String>, _>("authors").unwrap_or_default(),
                language: r.get("language"),
                isbn: r.get("isbn"),
                publish_date: r.get("publish_date"),
                abs_path: r.get("abs_path"),
                archived_at: r.get("archived_at"),
                reading_progress,
            }
        })
        .collect();

    Ok(Json(ArchivedSeriesDetail {
        id: row.get("id"),
        name: row.get("name"),
        library_id: row.get("library_id"),
        description: row.get("description"),
        authors: row.try_get::<Vec<String>, _>("authors").unwrap_or_default(),
        publishers: row.try_get::<Vec<String>, _>("publishers").unwrap_or_default(),
        genres: row.try_get::<Vec<String>, _>("genres").unwrap_or_default(),
        cover_url: row.get("cover_url"),
        total_volumes: row.get("total_volumes"),
        status: row.get("status"),
        start_year: row.get("start_year"),
        archived_at: row.get("archived_at"),
        books,
    }))
}
