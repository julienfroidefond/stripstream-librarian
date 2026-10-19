use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::ApiError, reading::series_progress::SERIES_PROGRESS_AGGREGATES, state::AppState,
};

/// Reading progress and rating of one reader for a series (admin view).
#[derive(Serialize, sqlx::FromRow, ToSchema)]
pub struct SeriesUserActivity {
    #[schema(value_type = String)]
    pub user_id: Uuid,
    pub username: String,
    pub books_read: i64,
    pub books_reading: i64,
    pub books_total: i64,
    pub last_read_at: Option<String>,
    /// Half-star rating on the 1–10 scale, or null when the reader did not rate the series.
    pub rating: Option<i16>,
}

/// List every reader who read or rated a series (admin).
///
/// Used by the backoffice series page when no reader is selected: the admin
/// sees all users' progress and ratings instead of a single-user view.
#[utoipa::path(
    get,
    path = "/admin/series/{series_id}/activity",
    tag = "series",
    params(("series_id" = String, Path, description = "Series UUID")),
    responses(
        (status = 200, body = Vec<SeriesUserActivity>),
        (status = 404, description = "Series not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_series_user_activity(
    State(state): State<AppState>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<Vec<SeriesUserActivity>>, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
        .bind(series_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError::not_found("series not found"));
    }

    let sql = format!(
        r#"
        WITH progress AS (
            SELECT
                brp.user_id,{SERIES_PROGRESS_AGGREGATES}
            FROM book_reading_progress brp
            JOIN books b ON b.id = brp.book_id
            WHERE b.series_id = $1
            GROUP BY brp.user_id
        ),
        totals AS (
            SELECT COUNT(*) AS books_total FROM books WHERE series_id = $1
        )
        SELECT
            u.id AS user_id,
            u.username,
            COALESCE(p.books_read, 0) AS books_read,
            COALESCE(p.books_reading, 0) AS books_reading,
            t.books_total,
            TO_CHAR(p.last_read_at, 'YYYY-MM-DD') AS last_read_at,
            sur.rating
        FROM users u
        CROSS JOIN totals t
        LEFT JOIN progress p ON p.user_id = u.id
        LEFT JOIN series_user_ratings sur
            ON sur.series_id = $1 AND sur.user_id = u.id
        WHERE COALESCE(p.books_read, 0) + COALESCE(p.books_reading, 0) > 0
           OR sur.rating IS NOT NULL
        ORDER BY p.last_read_at DESC NULLS LAST, u.username ASC
        "#
    );

    // Only compile-time fragments are interpolated, never user input.
    let rows = sqlx::query_as::<_, SeriesUserActivity>(sqlx::AssertSqlSafe(sql))
        .bind(series_id)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(rows))
}
