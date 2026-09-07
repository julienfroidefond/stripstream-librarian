use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

use super::SeriesItem;

#[utoipa::path(
    get,
    path = "/favorites",
    tag = "series",
    responses((status = 200, body = Vec<SeriesItem>)),
    security(("Bearer" = []))
)]
pub async fn list_favorites(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
) -> Result<Json<Vec<SeriesItem>>, ApiError> {
    let user_id = user.map(|u| u.0.user_id).ok_or_else(|| {
        ApiError::bad_request("no user selected — choose a user in the backoffice settings")
    })?;

    let rows = sqlx::query(
        r#"
        SELECT s.name, s.id AS series_id, s.library_id, s.status AS series_status,
               COUNT(b.id)::bigint AS book_count,
               COUNT(brp.book_id) FILTER (WHERE brp.status = 'read')::bigint AS books_read_count,
               fb.id AS first_book_id, fb.updated_at AS first_book_updated_at,
               ml.provider AS metadata_provider, asl.anilist_id, asl.anilist_url,
               s.cover_url, s.start_year, COALESCE(s.genres, ARRAY[]::text[]) AS genres,
               COALESCE(s.authors, ARRAY[]::text[]) AS authors, s.description,
               sur.rating AS user_rating
        FROM series_user_favorites f
        JOIN series s ON s.id = f.series_id
        LEFT JOIN books b ON b.series_id = s.id
        LEFT JOIN book_reading_progress brp ON brp.book_id = b.id AND brp.user_id = $1
        LEFT JOIN LATERAL (
            SELECT id, updated_at FROM books
            WHERE series_id = s.id
            ORDER BY CASE WHEN volume_type = 'regular' THEN 0 ELSE 1 END, volume NULLS LAST, title
            LIMIT 1
        ) fb ON TRUE
        LEFT JOIN LATERAL (
            SELECT provider FROM external_metadata_links
            WHERE series_id = s.id AND status = 'approved'
            ORDER BY created_at DESC LIMIT 1
        ) ml ON TRUE
        LEFT JOIN anilist_series_links asl ON asl.series_id = s.id AND asl.provider = 'anilist'
        LEFT JOIN series_user_ratings sur ON sur.series_id = s.id AND sur.user_id = $1
        WHERE f.user_id = $1
          AND NOT EXISTS (
              SELECT 1 FROM user_genre_restrictions ugr
              WHERE ugr.user_id = $1 AND ugr.genre = ANY(s.genres)
          )
        GROUP BY f.created_at, s.id, s.name, s.library_id, s.status, fb.id, fb.updated_at,
                 ml.provider, asl.anilist_id, asl.anilist_url, sur.rating
        ORDER BY f.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows.iter().map(series_item_from_row).collect()))
}

#[utoipa::path(
    put,
    path = "/series/{series_id}/favorite",
    tag = "series",
    security(("Bearer" = []))
)]
pub async fn add_favorite(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = user
        .map(|u| u.0.user_id)
        .ok_or_else(|| ApiError::bad_request("no user selected"))?;
    sqlx::query(
        "INSERT INTO series_user_favorites (user_id, series_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(series_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/series/{series_id}/favorite",
    tag = "series",
    security(("Bearer" = []))
)]
pub async fn remove_favorite(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = user
        .map(|u| u.0.user_id)
        .ok_or_else(|| ApiError::bad_request("no user selected"))?;
    sqlx::query("DELETE FROM series_user_favorites WHERE user_id = $1 AND series_id = $2")
        .bind(user_id)
        .bind(series_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/series/{series_id}/favorite",
    tag = "series",
    security(("Bearer" = []))
)]
pub async fn is_favorite(
    State(state): State<AppState>,
    user: Option<Extension<AuthUser>>,
    Path(series_id): Path<Uuid>,
) -> Result<Json<bool>, ApiError> {
    let user_id = user
        .map(|u| u.0.user_id)
        .ok_or_else(|| ApiError::bad_request("no user selected"))?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM series_user_favorites WHERE user_id = $1 AND series_id = $2)",
    )
    .bind(user_id)
    .bind(series_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(exists))
}

fn series_item_from_row(row: &sqlx::postgres::PgRow) -> SeriesItem {
    SeriesItem {
        name: row.get("name"),
        series_id: row.get("series_id"),
        book_count: row.get("book_count"),
        books_read_count: row.get("books_read_count"),
        first_book_id: row.get("first_book_id"),
        first_book_updated_at: row.get("first_book_updated_at"),
        library_id: row.get("library_id"),
        series_status: row.get("series_status"),
        missing_count: None,
        metadata_provider: row.get("metadata_provider"),
        anilist_id: row.get("anilist_id"),
        anilist_url: row.get("anilist_url"),
        cover_url: row.get("cover_url"),
        start_year: row.get("start_year"),
        genres: row.get("genres"),
        authors: row.get("authors"),
        description: row.get("description"),
        user_rating: row.get("user_rating"),
        community_score: None,
    }
}
