use axum::extract::Extension;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use sqlx::Row;
use uuid::Uuid;

use super::catalog::{self, SeriesRequest};
use super::{helpers, require_user, OngoingQuery, SeriesItem, SeriesMetadata};
use crate::books::files::current_book_file_join;
use crate::users::genre_restrictions::ensure_series_visible;
use crate::{auth::AuthUser, books::BookItem, error::ApiError, state::AppState};

// ─── Ongoing series / books ──────────────────────────────────────────────────

/// List ongoing series (partially read, sorted by most recent activity)
#[utoipa::path(
    get,
    path = "/series/ongoing",
    tag = "series",
    params(
        ("limit" = Option<i64>, Query, description = "Max items to return (default 10, max 50)"),
    ),
    responses(
        (status = 200, body = Vec<SeriesItem>),
        (status = 400, description = "No user selected"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn ongoing_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<OngoingQuery>,
) -> Result<Json<Vec<SeriesItem>>, ApiError> {
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let request = SeriesRequest::for_ongoing(require_user(user)?, limit);

    Ok(Json(catalog::query(&state.pool, request).await?.items))
}

/// List next unread book for each ongoing series (sorted by most recent activity)
#[utoipa::path(
    get,
    path = "/books/ongoing",
    tag = "series",
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
    user: Option<Extension<AuthUser>>,
    Query(query): Query<OngoingQuery>,
) -> Result<Json<Vec<BookItem>>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    // Without a user there is no reading progress → always empty
    let Some(uid) = user_id else {
        return Ok(Json(vec![]));
    };

    let tome_counts = helpers::series_tome_counts("up");

    let sql = format!(
        r#"
        -- Start from user's reading progress (small set) instead of scanning all books/series
        WITH user_progress AS (
            SELECT book_id, status, last_read_at, current_page
            FROM book_reading_progress
            WHERE user_id = $2
        ),
        active_series AS (
            SELECT DISTINCT b.series_id
            FROM user_progress up
            JOIN books b ON b.id = up.book_id
            WHERE up.status IN ('read', 'reading')
        ),
        series_stats AS (
            SELECT
                a_s.series_id,
                {tome_counts},
                MAX(up.last_read_at) AS series_last_read_at
            FROM active_series a_s
            JOIN books b ON b.series_id = a_s.series_id
            LEFT JOIN user_progress up ON up.book_id = b.id
            GROUP BY a_s.series_id
        ),
        ongoing_series AS (
            SELECT series_id, series_last_read_at
            FROM series_stats
            WHERE books_started_count > 0
              AND books_read_count < book_count
        ),
        next_books AS (
            SELECT
                b.id, b.library_id, b.kind, b.format, b.title, b.author, b.authors,
                s.name AS series, b.series_id, b.volume, b.volume_type,
                b.language, b.page_count, b.thumbnail_path, b.updated_at,
                b.summary, b.isbn, b.publish_date, bf.size_bytes,
                COALESCE(up.status, 'unread') AS reading_status,
                up.current_page AS reading_current_page,
                up.last_read_at AS reading_last_read_at,
                os.series_last_read_at,
                ROW_NUMBER() OVER (
                    PARTITION BY b.series_id
                    ORDER BY b.volume NULLS LAST, b.title
                ) AS rn
            FROM books b
            JOIN ongoing_series os ON b.series_id = os.series_id
            JOIN series s ON s.id = b.series_id
            {current_file}
            LEFT JOIN user_progress up ON up.book_id = b.id
            WHERE COALESCE(up.status, 'unread') != 'read'
              -- Specials never become the next tome, unless the series has nothing else.
              AND (
                  b.volume_type <> 'hs'
                  OR NOT EXISTS (
                      SELECT 1 FROM books bx
                      WHERE bx.series_id = b.series_id AND bx.volume_type <> 'hs'
                  )
              )
              AND NOT EXISTS (
                  SELECT 1 FROM user_genre_restrictions ugr
                  WHERE ugr.user_id = $2 AND ugr.genre = ANY(s.genres)
              )
              AND NOT EXISTS (
                  SELECT 1 FROM series_user_shelves sus
                  WHERE sus.user_id = $2 AND sus.series_id = s.id
              )
        )
        SELECT id, library_id, kind, format, title, author, authors, series, series_id, volume, volume_type,
               language, page_count, thumbnail_path, updated_at, summary, isbn, publish_date, size_bytes,
               reading_status, reading_current_page, reading_last_read_at
        FROM next_books
        WHERE rn = 1
        ORDER BY series_last_read_at DESC NULLS LAST
        LIMIT $1
        "#,
        current_file = current_book_file_join!("size_bytes"),
    );

    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(limit)
        .bind(uid)
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
                series_id: row.get("series_id"),
                volume: row.get("volume"),
                volume_type: row.get("volume_type"),
                language: row.get("language"),
                page_count: row.get("page_count"),
                thumbnail_url: thumbnail_path
                    .map(|_| format!("/books/{}/thumbnail", row.get::<Uuid, _>("id"))),
                updated_at: row.get("updated_at"),
                reading_status: row.get("reading_status"),
                reading_current_page: row.get("reading_current_page"),
                reading_last_read_at: row.get("reading_last_read_at"),
                file_size: row.get("size_bytes"),
                summary: row.get("summary"),
                isbn: row.get("isbn"),
                publish_date: row.get("publish_date"),
            }
        })
        .collect();

    Ok(Json(items))
}

// ─── Series metadata ────────────────────────────────────────────────────────

/// Get metadata for a specific series (deprecated: use GET /series/{series_id}/metadata)
#[deprecated]
#[utoipa::path(
    get,
    path = "/libraries/{library_id}/series/{series_id}/metadata",
    tag = "series (deprecated)",
    params(
        ("library_id" = String, Path, description = "Library UUID"),
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = SeriesMetadata),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_metadata(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    ensure_series_visible(&state.pool, user.map(|u| u.0.user_id), series_id).await?;
    // Fetch series row (contains metadata directly)
    let series_row = sqlx::query(
        "SELECT name, authors, genres, description, publishers, start_year, total_volumes, status, locked_fields, book_author, book_language \
         FROM series WHERE id = $1 AND library_id = $2"
    )
    .bind(series_id)
    .bind(library_id)
    .fetch_optional(&state.pool)
    .await?;

    // Fallback: get book_author/book_language from first book if not on series row
    let books_row = sqlx::query("SELECT author, language FROM books WHERE series_id = $1 LIMIT 1")
        .bind(series_id)
        .fetch_optional(&state.pool)
        .await?;

    Ok(Json(SeriesMetadata {
        series_name: series_row
            .as_ref()
            .map(|r| r.get::<String, _>("name"))
            .unwrap_or_default(),
        authors: series_row
            .as_ref()
            .map(|r| r.get::<Vec<String>, _>("authors"))
            .unwrap_or_default(),
        genres: series_row
            .as_ref()
            .map(|r| r.get::<Vec<String>, _>("genres"))
            .unwrap_or_default(),
        description: series_row.as_ref().and_then(|r| r.get("description")),
        publishers: series_row
            .as_ref()
            .map(|r| r.get::<Vec<String>, _>("publishers"))
            .unwrap_or_default(),
        start_year: series_row.as_ref().and_then(|r| r.get("start_year")),
        total_volumes: series_row.as_ref().and_then(|r| r.get("total_volumes")),
        status: series_row.as_ref().and_then(|r| r.get("status")),
        book_author: series_row
            .as_ref()
            .and_then(|r| r.get::<Option<String>, _>("book_author"))
            .or_else(|| books_row.as_ref().and_then(|r| r.get("author"))),
        book_language: series_row
            .as_ref()
            .and_then(|r| r.get::<Option<String>, _>("book_language"))
            .or_else(|| books_row.as_ref().and_then(|r| r.get("language"))),
        locked_fields: series_row
            .as_ref()
            .map(|r| r.get::<serde_json::Value, _>("locked_fields"))
            .unwrap_or(serde_json::json!({})),
    }))
}

// ─── Direct series-by-ID endpoints (no library_id in path) ──────────────────

/// Get a series by its UUID (resolves library_id internally)
#[utoipa::path(
    get,
    path = "/series/{series_id}/details",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = SeriesItem),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_by_id(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesItem>, ApiError> {
    let request = SeriesRequest::for_one(series_id, user.map(|u| u.0.user_id));

    catalog::query(&state.pool, request)
        .await?
        .items
        .into_iter()
        .next()
        .map(Json)
        .ok_or_else(|| ApiError::not_found("series not found"))
}

/// Get metadata for a series by its UUID (resolves library_id internally)
#[utoipa::path(
    get,
    path = "/series/{series_id}/metadata",
    tag = "series",
    params(
        ("series_id" = String, Path, description = "Series UUID"),
    ),
    responses(
        (status = 200, body = SeriesMetadata),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Series not found"),
    ),
    security(("Bearer" = []))
)]
#[allow(deprecated)]
pub async fn get_series_metadata_by_id(
    state: State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    let library_id = helpers::resolve_library_id(&state.pool, series_id).await?;
    get_series_metadata(state, user, Path((library_id, series_id))).await
}
