use axum::extract::Extension;
use axum::{extract::{Path, Query, State}, Json};
use sqlx::Row;
use uuid::Uuid;

use crate::{auth::AuthUser, books::BookItem, error::ApiError, state::AppState};
use super::{helpers, OngoingQuery, SeriesItem, SeriesMetadata};

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
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn ongoing_series(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Query(query): Query<OngoingQuery>,
) -> Result<Json<Vec<SeriesItem>>, ApiError> {
    let user_id: Option<uuid::Uuid> = user.map(|u| u.0.user_id);
    let limit = query.limit.unwrap_or(10).clamp(1, 50);

    let rows = sqlx::query(
        r#"
        WITH series_stats AS (
            SELECT
                s.id AS series_id,
                s.name,
                s.library_id,
                COUNT(*) AS book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') AS books_read_count,
                MAX(brp.last_read_at) AS last_read_at
            FROM series s
            JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            GROUP BY s.id, s.name, s.library_id
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        first_books AS (
            SELECT
                b.series_id,
                b.id,
                b.library_id,
                b.updated_at,
                ROW_NUMBER() OVER (
                    PARTITION BY b.series_id
                    ORDER BY
                        b.volume NULLS LAST,
                        REGEXP_REPLACE(LOWER(b.title), '[0-9].*$', ''),
                        COALESCE((REGEXP_MATCH(LOWER(b.title), '\d+'))[1]::int, 0),
                        b.title ASC
                ) AS rn
            FROM books b
        )
        SELECT ss.name, ss.series_id, ss.book_count, ss.books_read_count, fb.id AS first_book_id, fb.updated_at AS first_book_updated_at, fb.library_id
        FROM series_stats ss
        JOIN first_books fb ON fb.series_id = ss.series_id AND fb.rn = 1
        ORDER BY ss.last_read_at DESC NULLS LAST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<SeriesItem> = rows
        .iter()
        .map(|row| SeriesItem {
            name: row.get("name"),
            series_id: row.get("series_id"),
            book_count: row.get("book_count"),
            books_read_count: row.get("books_read_count"),
            first_book_id: row.get("first_book_id"),
            first_book_updated_at: row.get("first_book_updated_at"),
            library_id: row.get("library_id"),
            series_status: None,
            missing_count: None,
            metadata_provider: None,
            anilist_id: None,
            anilist_url: None,
            cover_url: None,
        })
        .collect();

    Ok(Json(items))
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

    let rows = sqlx::query(
        r#"
        WITH ongoing_series AS (
            SELECT
                s.id AS series_id,
                MAX(brp.last_read_at) AS series_last_read_at
            FROM series s
            JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            GROUP BY s.id
            HAVING (
                COUNT(brp.book_id) FILTER (WHERE brp.status IN ('read', 'reading')) > 0
                AND COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') < COUNT(*)
            )
        ),
        next_books AS (
            SELECT
                b.id, b.library_id, b.kind, b.format, b.title, b.author, b.authors, s.name AS series, b.volume, b.volume_type,
                b.language, b.page_count, b.thumbnail_path, b.updated_at,
                COALESCE(brp.status, 'unread') AS reading_status,
                brp.current_page AS reading_current_page,
                brp.last_read_at AS reading_last_read_at,
                os.series_last_read_at,
                ROW_NUMBER() OVER (
                    PARTITION BY b.series_id
                    ORDER BY b.volume NULLS LAST, b.title
                ) AS rn
            FROM books b
            JOIN ongoing_series os ON b.series_id = os.series_id
            JOIN series s ON s.id = b.series_id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            WHERE COALESCE(brp.status, 'unread') != 'read'
        )
        SELECT id, library_id, kind, format, title, author, authors, series, volume, volume_type, language, page_count,
               thumbnail_path, updated_at, reading_status, reading_current_page, reading_last_read_at
        FROM next_books
        WHERE rn = 1
        ORDER BY series_last_read_at DESC NULLS LAST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .bind(user_id)
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
                volume_type: row.get("volume_type"),
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
    Path((library_id, series_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    // Fetch series row (contains metadata directly)
    let series_row = sqlx::query(
        "SELECT name, authors, description, publishers, start_year, total_volumes, status, locked_fields, book_author, book_language \
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
        series_name: series_row.as_ref().map(|r| r.get::<String, _>("name")).unwrap_or_default(),
        authors: series_row.as_ref().map(|r| r.get::<Vec<String>, _>("authors")).unwrap_or_default(),
        description: series_row.as_ref().and_then(|r| r.get("description")),
        publishers: series_row.as_ref().map(|r| r.get::<Vec<String>, _>("publishers")).unwrap_or_default(),
        start_year: series_row.as_ref().and_then(|r| r.get("start_year")),
        total_volumes: series_row.as_ref().and_then(|r| r.get("total_volumes")),
        status: series_row.as_ref().and_then(|r| r.get("status")),
        book_author: series_row.as_ref().and_then(|r| r.get::<Option<String>, _>("book_author"))
            .or_else(|| books_row.as_ref().and_then(|r| r.get("author"))),
        book_language: series_row.as_ref().and_then(|r| r.get::<Option<String>, _>("book_language"))
            .or_else(|| books_row.as_ref().and_then(|r| r.get("language"))),
        locked_fields: series_row.as_ref().map(|r| r.get::<serde_json::Value, _>("locked_fields")).unwrap_or(serde_json::json!({})),
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
    let user_id: Option<Uuid> = user.map(|u| u.0.user_id);

    let row = sqlx::query(
        r#"
        WITH series_counts AS (
            SELECT
                s.id as series_id, s.name, s.library_id, s.status as series_status,
                COUNT(b.id) as book_count,
                COUNT(brp.book_id) FILTER (WHERE brp.status = 'read') as books_read_count
            FROM series s
            LEFT JOIN books b ON b.series_id = s.id
            LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND $2::uuid IS NOT NULL AND brp.user_id = $2
            WHERE s.id = $1
            GROUP BY s.id, s.name, s.library_id, s.status
        ),
        first_book AS (
            SELECT b.id, b.series_id, b.updated_at
            FROM books b WHERE b.series_id = $1
            ORDER BY b.volume NULLS LAST, b.title ASC
            LIMIT 1
        )
        SELECT sc.name, sc.series_id, sc.book_count, sc.books_read_count,
               COALESCE(fb.id, '00000000-0000-0000-0000-000000000000'::uuid) as first_book_id,
               fb.updated_at as first_book_updated_at,
               sc.library_id, sc.series_status,
               mc.missing_count,
               ml.provider as metadata_provider,
               asl.anilist_id, asl.anilist_url,
               s.cover_url
        FROM series_counts sc
        LEFT JOIN series s ON s.id = sc.series_id
        LEFT JOIN first_book fb ON fb.series_id = sc.series_id
        LEFT JOIN (
            SELECT s2.id as series_id,
                CASE
                    WHEN COUNT(b2.id) FILTER (WHERE b2.volume_type = 'integral') > 0 THEN 0
                    ELSE GREATEST(COALESCE(s2.total_volumes, 0) - COUNT(b2.id) FILTER (WHERE b2.volume_type IN ('regular', 'integral')), 0)
                END as missing_count
            FROM series s2
            LEFT JOIN books b2 ON b2.series_id = s2.id
            WHERE s2.id = $1
            GROUP BY s2.id
        ) mc ON mc.series_id = sc.series_id
        LEFT JOIN (
            SELECT DISTINCT ON (eml.series_id) eml.series_id, eml.provider
            FROM external_metadata_links eml
            WHERE eml.series_id = $1 AND eml.status = 'approved'
            ORDER BY eml.series_id, eml.created_at DESC
        ) ml ON ml.series_id = sc.series_id
        LEFT JOIN anilist_series_links asl ON asl.series_id = sc.series_id AND asl.provider = 'anilist'
        "#
    )
    .bind(series_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::not_found("series not found"))?;

    Ok(Json(SeriesItem {
        name: row.get("name"),
        series_id: row.get("series_id"),
        book_count: row.get("book_count"),
        books_read_count: row.get("books_read_count"),
        first_book_id: row.get("first_book_id"),
        first_book_updated_at: row.get("first_book_updated_at"),
        library_id: row.get("library_id"),
        series_status: row.get("series_status"),
        missing_count: row.get("missing_count"),
        metadata_provider: row.get("metadata_provider"),
        anilist_id: row.get("anilist_id"),
        anilist_url: row.get("anilist_url"),
        cover_url: row.get("cover_url"),
    }))
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
    _user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<SeriesMetadata>, ApiError> {
    let library_id = helpers::resolve_library_id(&state.pool, series_id).await?;
    get_series_metadata(state, Path((library_id, series_id))).await
}
