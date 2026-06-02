use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

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

    let rows = sqlx::query(
        "SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre",
    )
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

    let rows = sqlx::query(
        "SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre",
    )
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

    let rows = sqlx::query(
        "SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre",
    )
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

    let rows = sqlx::query(
        "SELECT genre FROM user_genre_restrictions WHERE user_id = $1 ORDER BY genre",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let blocked_genres = rows.into_iter().map(|r| r.get("genre")).collect();
    Ok(Json(UserGenreRestrictionsResponse { blocked_genres }))
}
