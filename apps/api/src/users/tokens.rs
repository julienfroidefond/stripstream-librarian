use argon2::{Argon2, PasswordHasher};
use axum::{
    extract::{Path, State},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use rand::{rngs::SysRng, TryRng};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct CreateTokenRequest {
    #[schema(value_type = String, example = "My API Token")]
    pub name: String,
    #[schema(value_type = Option<String>, example = "read")]
    pub scope: Option<String>,
    #[schema(value_type = Option<String>)]
    pub user_id: Option<Uuid>,
}

#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub prefix: String,
    #[schema(value_type = Option<String>)]
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    #[schema(value_type = Option<String>)]
    pub last_used_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>)]
    pub revoked_at: Option<DateTime<Utc>>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedTokenResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub token: String,
    pub prefix: String,
}

/// Create a new API token with read or admin scope. The token is only shown once.
#[utoipa::path(
    post,
    path = "/admin/tokens",
    tag = "tokens",
    request_body = CreateTokenRequest,
    responses(
        (status = 200, body = CreatedTokenResponse, description = "Token created - token is only shown once"),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn create_token(
    State(state): State<AppState>,
    Json(input): Json<CreateTokenRequest>,
) -> Result<Json<CreatedTokenResponse>, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::bad_request("name is required"));
    }

    let scope = match input.scope.as_deref().unwrap_or("read") {
        "admin" => "admin",
        "read" => "read",
        _ => return Err(ApiError::bad_request("scope must be 'admin' or 'read'")),
    };

    if scope == "read" && input.user_id.is_none() {
        return Err(ApiError::bad_request(
            "user_id is required for read-scoped tokens",
        ));
    }

    let mut random = [0u8; 24];
    SysRng
        .try_fill_bytes(&mut random)
        .map_err(|e| ApiError::internal(format!("failed to generate token secret: {e}")))?;
    let secret = URL_SAFE_NO_PAD.encode(random);
    let prefix: String = secret.chars().take(8).collect();
    let token = format!("stl_{prefix}_{secret}");

    let token_hash = Argon2::default()
        .hash_password(token.as_bytes())
        .map_err(|_| ApiError::internal("failed to hash token"))?
        .to_string();

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO api_tokens (id, name, prefix, token_hash, scope, user_id) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(input.name.trim())
    .bind(&prefix)
    .bind(token_hash)
    .bind(scope)
    .bind(input.user_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(CreatedTokenResponse {
        id,
        name: input.name.trim().to_string(),
        scope: scope.to_string(),
        token,
        prefix,
    }))
}

/// List all API tokens
#[utoipa::path(
    get,
    path = "/admin/tokens",
    tag = "tokens",
    responses(
        (status = 200, body = Vec<TokenResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn list_tokens(
    State(state): State<AppState>,
) -> Result<Json<Vec<TokenResponse>>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT t.id, t.name, t.scope, t.prefix, t.user_id, u.username,
               t.last_used_at, t.revoked_at, t.created_at
        FROM api_tokens t
        LEFT JOIN users u ON u.id = t.user_id
        ORDER BY t.created_at DESC
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| TokenResponse {
            id: row.get("id"),
            name: row.get("name"),
            scope: row.get("scope"),
            prefix: row.get("prefix"),
            user_id: row.get("user_id"),
            username: row.get("username"),
            last_used_at: row.get("last_used_at"),
            revoked_at: row.get("revoked_at"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(items))
}

/// Revoke an API token by ID
#[utoipa::path(
    delete,
    path = "/admin/tokens/{id}",
    tag = "tokens",
    params(
        ("id" = String, Path, description = "Token UUID"),
    ),
    responses(
        (status = 200, description = "Token revoked"),
        (status = 404, description = "Token not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn revoke_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::responses::RevokedResponse>, ApiError> {
    let result = sqlx::query(
        "UPDATE api_tokens SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("token not found"));
    }

    Ok(Json(crate::responses::RevokedResponse::new(id)))
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateTokenRequest {
    #[schema(value_type = Option<String>)]
    pub user_id: Option<Uuid>,
}

/// Update a token's assigned user
#[utoipa::path(
    patch,
    path = "/admin/tokens/{id}",
    tag = "tokens",
    params(
        ("id" = String, Path, description = "Token UUID"),
    ),
    request_body = UpdateTokenRequest,
    responses(
        (status = 200, description = "Token updated"),
        (status = 404, description = "Token not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn update_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTokenRequest>,
) -> Result<Json<crate::responses::UpdatedResponse>, ApiError> {
    let result = sqlx::query("UPDATE api_tokens SET user_id = $1 WHERE id = $2")
        .bind(input.user_id)
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("token not found"));
    }

    Ok(Json(crate::responses::UpdatedResponse::new(id)))
}

/// Permanently delete a revoked API token
#[utoipa::path(
    post,
    path = "/admin/tokens/{id}/delete",
    tag = "tokens",
    params(
        ("id" = String, Path, description = "Token UUID"),
    ),
    responses(
        (status = 200, description = "Token permanently deleted"),
        (status = 404, description = "Token not found or not revoked"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - Admin scope required"),
    ),
    security(("Bearer" = []))
)]
pub async fn delete_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::responses::DeletedResponse>, ApiError> {
    let result = sqlx::query("DELETE FROM api_tokens WHERE id = $1 AND revoked_at IS NOT NULL")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("token not found or not revoked"));
    }

    Ok(Json(crate::responses::DeletedResponse::new(id)))
}

/// Build a token string in the canonical format: stl_{prefix}_{secret}
/// Extracted for testability — prefix is the first 8 chars of the base64-encoded secret.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_token(secret_bytes: &[u8]) -> (String, String) {
    let secret = URL_SAFE_NO_PAD.encode(secret_bytes);
    let prefix: String = secret.chars().take(8).collect();
    let token = format!("stl_{prefix}_{secret}");
    (token, prefix)
}

/// Validate a scope string: must be "admin" or "read".
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn validate_scope(scope: Option<&str>) -> Result<&'static str, &'static str> {
    match scope.unwrap_or("read") {
        "admin" => Ok("admin"),
        "read" => Ok("read"),
        _ => Err("scope must be 'admin' or 'read'"),
    }
}

#[cfg(test)]
#[path = "tests/tokens.rs"]
mod tests;
