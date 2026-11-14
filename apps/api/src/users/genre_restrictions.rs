use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

// ─── Access checks ──────────────────────────────────────────────────────────

/// Whether `user_id` may see the series `series_id`: an unrestricted caller
/// (`None`: admin token without impersonation) sees everything, a reader never
/// sees a series carrying one of their blocked genres. Same rule as the
/// `NOT EXISTS (… user_genre_restrictions …)` filters of the listings and of
/// `GET /series/{id}/details`. A missing series is reported as not visible.
pub(crate) async fn series_visible(
    pool: &PgPool,
    user_id: Option<Uuid>,
    series_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM series s
            WHERE s.id = $1
              AND ($2::uuid IS NULL OR NOT EXISTS (
                  SELECT 1 FROM user_genre_restrictions ugr
                  WHERE ugr.user_id = $2 AND ugr.genre = ANY(s.genres)
              ))
        )
        "#,
    )
    .bind(series_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// `404 series not found` unless [`series_visible`].
pub(crate) async fn ensure_series_visible(
    pool: &PgPool,
    user_id: Option<Uuid>,
    series_id: Uuid,
) -> Result<(), ApiError> {
    if series_visible(pool, user_id, series_id).await? {
        Ok(())
    } else {
        Err(ApiError::not_found("series not found"))
    }
}

/// `404 book not found` unless the book exists and its series (if any) is
/// visible to `user_id` (see [`series_visible`]). A book without series is
/// never restricted.
pub(crate) async fn ensure_book_visible(
    pool: &PgPool,
    user_id: Option<Uuid>,
    book_id: Uuid,
) -> Result<(), ApiError> {
    let visible: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM books b
            LEFT JOIN series s ON s.id = b.series_id
            WHERE b.id = $1
              AND ($2::uuid IS NULL OR NOT EXISTS (
                  SELECT 1 FROM user_genre_restrictions ugr
                  WHERE ugr.user_id = $2 AND ugr.genre = ANY(s.genres)
              ))
        )
        "#,
    )
    .bind(book_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if visible {
        Ok(())
    } else {
        Err(ApiError::not_found("book not found"))
    }
}

#[derive(Serialize, ToSchema)]
pub struct UserGenreRestrictionsResponse {
    pub blocked_genres: Vec<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddGenreRestrictionRequest {
    pub genre: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SetGenreRestrictionsRequest {
    pub blocked_genres: Vec<String>,
}

/// Get blocked genres for a user
#[utoipa::path(
    get,
    path = "/admin/users/{id}/genre-restrictions",
    tag = "users",
    params(("id" = String, Path, description = "User UUID")),
    responses(
        (status = 200, body = UserGenreRestrictionsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
        (status = 404, description = "User not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_genre_restrictions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserGenreRestrictionsResponse>, ApiError> {
    let exists = sqlx::query("SELECT 1 FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("user not found"));
    }

    let rows =
        sqlx::query("SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre")
            .bind(id)
            .fetch_all(&state.pool)
            .await?;

    let blocked_genres = rows.into_iter().map(|r| r.get("genre")).collect();
    Ok(Json(UserGenreRestrictionsResponse { blocked_genres }))
}

/// Add a blocked genre for a user
#[utoipa::path(
    post,
    path = "/admin/users/{id}/genre-restrictions",
    tag = "users",
    params(("id" = String, Path, description = "User UUID")),
    request_body = AddGenreRestrictionRequest,
    responses(
        (status = 200, body = UserGenreRestrictionsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
        (status = 404, description = "User not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn add_genre_restriction(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<AddGenreRestrictionRequest>,
) -> Result<Json<UserGenreRestrictionsResponse>, ApiError> {
    if input.genre.trim().is_empty() {
        return Err(ApiError::bad_request("genre is required"));
    }

    let exists = sqlx::query("SELECT 1 FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("user not found"));
    }

    sqlx::query(
        "INSERT INTO user_genre_restrictions (user_id, genre) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(input.genre.trim())
    .execute(&state.pool)
    .await?;

    let rows =
        sqlx::query("SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre")
            .bind(id)
            .fetch_all(&state.pool)
            .await?;

    let blocked_genres = rows.into_iter().map(|r| r.get("genre")).collect();
    Ok(Json(UserGenreRestrictionsResponse { blocked_genres }))
}

/// Remove a blocked genre for a user
#[utoipa::path(
    delete,
    path = "/admin/users/{id}/genre-restrictions/{genre}",
    tag = "users",
    params(
        ("id" = String, Path, description = "User UUID"),
        ("genre" = String, Path, description = "Genre to unblock"),
    ),
    responses(
        (status = 200, body = UserGenreRestrictionsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
        (status = 404, description = "User not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn remove_genre_restriction(
    State(state): State<AppState>,
    Path((id, genre)): Path<(Uuid, String)>,
) -> Result<Json<UserGenreRestrictionsResponse>, ApiError> {
    let exists = sqlx::query("SELECT 1 FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("user not found"));
    }

    sqlx::query("DELETE FROM user_genre_restrictions WHERE user_id = $1 AND genre = $2")
        .bind(id)
        .bind(&genre)
        .execute(&state.pool)
        .await?;

    let rows =
        sqlx::query("SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre")
            .bind(id)
            .fetch_all(&state.pool)
            .await?;

    let blocked_genres = rows.into_iter().map(|r| r.get("genre")).collect();
    Ok(Json(UserGenreRestrictionsResponse { blocked_genres }))
}

/// Replace all blocked genres for a user
#[utoipa::path(
    put,
    path = "/admin/users/{id}/genre-restrictions",
    tag = "users",
    params(("id" = String, Path, description = "User UUID")),
    request_body = SetGenreRestrictionsRequest,
    responses(
        (status = 200, body = UserGenreRestrictionsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
        (status = 404, description = "User not found"),
    ),
    security(("Bearer" = []))
)]
pub async fn set_genre_restrictions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<SetGenreRestrictionsRequest>,
) -> Result<Json<UserGenreRestrictionsResponse>, ApiError> {
    let exists = sqlx::query("SELECT 1 FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(ApiError::not_found("user not found"));
    }

    let mut tx = state.pool.begin().await?;

    sqlx::query("DELETE FROM user_genre_restrictions WHERE user_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    let genres: Vec<String> = input
        .blocked_genres
        .into_iter()
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty())
        .collect();

    if !genres.is_empty() {
        sqlx::query(
            "INSERT INTO user_genre_restrictions (user_id, genre) SELECT $1, UNNEST($2::text[])",
        )
        .bind(id)
        .bind(&genres)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let rows =
        sqlx::query("SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre")
            .bind(id)
            .fetch_all(&state.pool)
            .await?;

    let blocked_genres = rows.into_iter().map(|r| r.get("genre")).collect();
    Ok(Json(UserGenreRestrictionsResponse { blocked_genres }))
}

#[cfg(test)]
#[path = "tests/genre_restrictions.rs"]
mod tests;
