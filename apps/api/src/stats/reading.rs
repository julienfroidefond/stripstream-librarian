use axum::{
    extract::{Path, Query, State},
    Json,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

use super::types::*;

/// Get reading overview for all users (admin)
#[utoipa::path(
    get,
    path = "/admin/reading-overview",
    tag = "stats",
    responses(
        (status = 200, body = Vec<UserReadingOverview>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_reading_overview(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserReadingOverview>>, ApiError> {
    let rows = sqlx::query(
        r#"
        WITH series_counts AS (
            SELECT
                b.series_id,
                COALESCE(b.series_id, '00000000-0000-0000-0000-000000000000'::uuid) AS series_key,
                COALESCE(s.name, 'Sans série') AS series_name,
                COUNT(*) AS books_total
            FROM books b
            LEFT JOIN series s ON s.id = b.series_id
            GROUP BY b.series_id, s.name
        ),
        first_books AS (
            SELECT DISTINCT ON (COALESCE(b.series_id, '00000000-0000-0000-0000-000000000000'::uuid))
                COALESCE(b.series_id, '00000000-0000-0000-0000-000000000000'::uuid) AS series_key,
                b.id AS first_book_id
            FROM books b
            ORDER BY
                COALESCE(b.series_id, '00000000-0000-0000-0000-000000000000'::uuid),
                CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
                b.volume NULLS LAST,
                b.title
        ),
        series_totals AS (
            SELECT sc.*, fb.first_book_id
            FROM series_counts sc
            JOIN first_books fb USING (series_key)
        ),
        user_series AS (
            SELECT
                brp.user_id,
                COALESCE(b.series_id, '00000000-0000-0000-0000-000000000000'::uuid) AS series_key,
                COUNT(*) FILTER (WHERE brp.status = 'read') AS books_read,
                COUNT(*) FILTER (WHERE brp.status = 'reading') AS books_reading,
                MAX(brp.last_read_at) AS last_read_at
            FROM book_reading_progress brp
            JOIN books b ON b.id = brp.book_id
            GROUP BY brp.user_id, series_key
        )
        SELECT
            u.id AS user_id,
            u.username,
            COUNT(*) FILTER (WHERE brp.status = 'read') AS books_read,
            COUNT(*) FILTER (WHERE brp.status = 'reading') AS books_reading,
            COUNT(DISTINCT CASE WHEN brp.status = 'reading' THEN b.series_id END) AS series_in_progress,
            TO_CHAR(MAX(brp.last_read_at), 'YYYY-MM-DD') AS last_read_at,
            COALESCE(
                json_agg(
                    json_build_object(
                        'book_id', b.id::text,
                        'title', b.title,
                        'series', s.name,
                        'series_id', b.series_id::text,
                        'current_page', COALESCE(brp.current_page, 0),
                        'page_count', COALESCE(b.page_count, 0),
                        'last_read_at', TO_CHAR(brp.last_read_at, 'YYYY-MM-DD')
                    ) ORDER BY brp.updated_at DESC
                ) FILTER (WHERE brp.status = 'reading'),
                '[]'::json
            ) AS currently_reading,
            (
                SELECT COALESCE(json_agg(rr), '[]'::json)
                FROM (
                    SELECT json_build_object(
                        'book_id', b2.id::text,
                        'title', b2.title,
                        'series', s2.name,
                        'series_id', b2.series_id::text,
                        'current_page', 0,
                        'page_count', COALESCE(b2.page_count, 0),
                        'last_read_at', TO_CHAR(brp2.last_read_at, 'YYYY-MM-DD')
                    ) AS rr
                    FROM book_reading_progress brp2
                    JOIN books b2 ON b2.id = brp2.book_id
                    LEFT JOIN series s2 ON s2.id = b2.series_id
                    WHERE brp2.user_id = u.id AND brp2.status = 'read'
                    ORDER BY brp2.last_read_at DESC NULLS LAST
                    LIMIT 30
                ) sub
            ) AS recently_read,
            (
                SELECT COALESCE(
                    json_agg(
                        json_build_object(
                            'series_id', st.series_id::text,
                            'series_name', st.series_name,
                            'first_book_id', st.first_book_id::text,
                            'books_total', st.books_total,
                            'books_read', COALESCE(us.books_read, 0),
                            'books_reading', COALESCE(us.books_reading, 0),
                            'books_unread', st.books_total
                                - COALESCE(us.books_read, 0)
                                - COALESCE(us.books_reading, 0),
                            'last_read_at', TO_CHAR(us.last_read_at, 'YYYY-MM-DD')
                        )
                        ORDER BY us.last_read_at DESC NULLS LAST, st.series_name ASC
                    ),
                    '[]'::json
                )
                FROM series_totals st
                LEFT JOIN user_series us
                    ON us.user_id = u.id AND us.series_key = st.series_key
            ) AS series_progress
        FROM users u
        LEFT JOIN book_reading_progress brp ON brp.user_id = u.id
        LEFT JOIN books b ON b.id = brp.book_id
        LEFT JOIN series s ON s.id = b.series_id
        GROUP BY u.id, u.username
        ORDER BY MAX(brp.last_read_at) DESC NULLS LAST, u.username ASC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let overview = rows
        .into_iter()
        .map(|r| {
            let currently_json: serde_json::Value = r
                .try_get("currently_reading")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let currently_reading: Vec<UserReadingOverviewItem> =
                serde_json::from_value(currently_json).unwrap_or_default();
            let recently_json: serde_json::Value = r
                .try_get("recently_read")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let recently_read: Vec<UserReadingOverviewItem> =
                serde_json::from_value(recently_json).unwrap_or_default();
            let series_json: serde_json::Value = r
                .try_get("series_progress")
                .unwrap_or(serde_json::Value::Array(vec![]));
            let series_progress: Vec<UserReadingOverviewSeries> =
                serde_json::from_value(series_json).unwrap_or_default();
            UserReadingOverview {
                user_id: r.get("user_id"),
                username: r.get("username"),
                books_read: r.get("books_read"),
                books_reading: r.get("books_reading"),
                series_in_progress: r.get("series_in_progress"),
                last_read_at: r.get("last_read_at"),
                currently_reading,
                recently_read,
                series_progress,
            }
        })
        .collect();

    Ok(Json(overview))
}

/// List the books of one series with a user's reading status (admin).
///
/// Loaded on demand when a series is expanded in the reading overview.
#[utoipa::path(
    get,
    path = "/admin/reading-overview/{user_id}/series-books",
    tag = "stats",
    params(
        ("user_id" = String, Path, description = "User UUID"),
        ReadingOverviewSeriesBooksQuery,
    ),
    responses(
        (status = 200, body = Vec<UserReadingOverviewSeriesBook>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_reading_overview_series_books(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ReadingOverviewSeriesBooksQuery>,
) -> Result<Json<Vec<UserReadingOverviewSeriesBook>>, ApiError> {
    // `= $2 OR (… IS NULL)` rather than `IS NOT DISTINCT FROM`, which can't use the series index.
    let books = sqlx::query_as::<_, UserReadingOverviewSeriesBook>(
        r#"
        SELECT
            b.id::text AS book_id,
            b.title,
            b.volume,
            b.volume_type,
            COALESCE(brp.status, 'unread') AS status,
            COALESCE(brp.current_page, 0) AS current_page,
            COALESCE(b.page_count, 0) AS page_count,
            TO_CHAR(brp.last_read_at, 'YYYY-MM-DD') AS last_read_at
        FROM books b
        LEFT JOIN book_reading_progress brp
            ON brp.book_id = b.id AND brp.user_id = $1
        WHERE b.series_id = $2 OR ($2::uuid IS NULL AND b.series_id IS NULL)
        ORDER BY
            CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
            b.volume NULLS LAST,
            b.title
        "#,
    )
    .bind(user_id)
    .bind(query.series_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(books))
}
