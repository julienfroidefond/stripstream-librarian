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

#[derive(Serialize, ToSchema)]
pub struct UserResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub username: String,
    pub token_count: i64,
    pub books_read: i64,
    pub books_reading: i64,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub username: String,
}

/// List all reader users with their associated token count
#[utoipa::path(
    get,
    path = "/admin/users",
    tag = "users",
    responses(
        (status = 200, body = Vec<UserResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_users(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserResponse>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT u.id, u.username, u.created_at,
               COUNT(DISTINCT t.id) AS token_count,
               COUNT(DISTINCT brp.book_id) FILTER (WHERE brp.status = 'read')    AS books_read,
               COUNT(DISTINCT brp.book_id) FILTER (WHERE brp.status = 'reading') AS books_reading
        FROM users u
        LEFT JOIN api_tokens t      ON t.user_id = u.id AND t.revoked_at IS NULL
        LEFT JOIN book_reading_progress brp ON brp.user_id = u.id
        GROUP BY u.id, u.username, u.created_at
        ORDER BY u.created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| UserResponse {
            id: row.get("id"),
            username: row.get("username"),
            token_count: row.get("token_count"),
            books_read: row.get("books_read"),
            books_reading: row.get("books_reading"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(items))
}

/// Create a new reader user
#[utoipa::path(
    post,
    path = "/admin/users",
    tag = "users",
    request_body = CreateUserRequest,
    responses(
        (status = 200, body = UserResponse, description = "User created"),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn create_user(
    State(state): State<AppState>,
    Json(input): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    if input.username.trim().is_empty() {
        return Err(ApiError::bad_request("username is required"));
    }

    let id = Uuid::new_v4();
    let row = sqlx::query(
        "INSERT INTO users (id, username) VALUES ($1, $2) RETURNING id, username, created_at",
    )
    .bind(id)
    .bind(input.username.trim())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.constraint() == Some("users_username_key") {
                return ApiError::bad_request("username already exists");
            }
        }
        ApiError::from(e)
    })?;

    Ok(Json(UserResponse {
        id: row.get("id"),
        username: row.get("username"),
        token_count: 0,
        books_read: 0,
        books_reading: 0,
        created_at: row.get("created_at"),
    }))
}

/// Update a reader user's username
#[utoipa::path(
    patch,
    path = "/admin/users/{id}",
    tag = "users",
    request_body = CreateUserRequest,
    responses(
        (status = 200, body = UserResponse, description = "User updated"),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "User not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateUserRequest>,
) -> Result<Json<crate::responses::UpdatedResponse>, ApiError> {
    if input.username.trim().is_empty() {
        return Err(ApiError::bad_request("username is required"));
    }

    let result = sqlx::query("UPDATE users SET username = $1 WHERE id = $2")
        .bind(input.username.trim())
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("users_username_key") {
                    return ApiError::bad_request("username already exists");
                }
            }
            ApiError::from(e)
        })?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("user not found"));
    }

    Ok(Json(crate::responses::UpdatedResponse::new(id)))
}

/// Delete a reader user (cascades on tokens and reading progress)
#[utoipa::path(
    delete,
    path = "/admin/users/{id}",
    tag = "users",
    params(
        ("id" = String, Path, description = "User UUID"),
    ),
    responses(
        (status = 200, description = "User deleted"),
        (status = 404, description = "User not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("user not found"));
    }

    Ok(Json(crate::responses::DeletedResponse::new(id)))
}
