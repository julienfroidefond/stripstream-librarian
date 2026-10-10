use axum::{extract::State, Json};
use sqlx::Row;

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
        WITH series_books AS (
            SELECT
                b.series_id,
                COALESCE(s.name, 'Sans série') AS series_name,
                json_agg(
                    json_build_object(
                        'book_id', b.id::text,
                        'title', b.title,
                        'volume', b.volume,
                        'volume_type', b.volume_type,
                        'page_count', COALESCE(b.page_count, 0)
                    )
                    ORDER BY
                        CASE WHEN b.volume_type = 'regular' THEN 0 ELSE 1 END,
                        b.volume NULLS LAST,
                        b.title
                ) AS books
            FROM books b
            LEFT JOIN series s ON s.id = b.series_id
            GROUP BY b.series_id, s.name
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
                    json_agg(series_row ORDER BY sort_last_read_at DESC NULLS LAST, series_name ASC),
                    '[]'::json
                )
                FROM (
                    SELECT
                        sb.series_name,
                        MAX(up.last_read_at) AS sort_last_read_at,
                        json_build_object(
                            'series_id', sb.series_id::text,
                            'series_name', sb.series_name,
                            'books_total', COUNT(*),
                            'books_read', COUNT(*) FILTER (WHERE COALESCE(up.status, 'unread') = 'read'),
                            'books_reading', COUNT(*) FILTER (WHERE COALESCE(up.status, 'unread') = 'reading'),
                            'books_unread', COUNT(*) FILTER (WHERE COALESCE(up.status, 'unread') = 'unread'),
                            'last_read_at', TO_CHAR(MAX(up.last_read_at), 'YYYY-MM-DD'),
                            'books', json_agg(
                                json_build_object(
                                    'book_id', bk->>'book_id',
                                    'title', bk->>'title',
                                    'volume', bk->'volume',
                                    'volume_type', bk->>'volume_type',
                                    'status', COALESCE(up.status, 'unread'),
                                    'current_page', COALESCE(up.current_page, 0),
                                    'page_count', (bk->>'page_count')::int,
                                    'last_read_at', TO_CHAR(up.last_read_at, 'YYYY-MM-DD')
                                )
                                ORDER BY
                                    CASE WHEN bk->>'volume_type' = 'regular' THEN 0 ELSE 1 END,
                                    (bk->>'volume')::int NULLS LAST,
                                    bk->>'title'
                            )
                        ) AS series_row
                    FROM series_books sb
                    CROSS JOIN LATERAL json_array_elements(sb.books) AS bk
                    LEFT JOIN book_reading_progress up
                        ON up.book_id = (bk->>'book_id')::uuid AND up.user_id = u.id
                    GROUP BY sb.series_id, sb.series_name
                ) series_rows
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
