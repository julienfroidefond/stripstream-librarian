use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use axum::{extract::{Path, State}, Json};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

use crate::{error::ApiError, AppState};

#[derive(Deserialize, ToSchema)]
pub struct CreateTokenRequest {
    #[schema(value_type = String, example = "My API Token")]
    pub name: String,
    #[schema(value_type = Option<String>, example = "read")]
    pub scope: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub prefix: String,
    #[schema(value_type = Option<String>)]
    pub last_used_at: Option<DateTime<Utc>>,
    #[schema(value_type = Option<String>)]
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedTokenResponse {
    pub id: Uuid,
    pub name: String,
    pub scope: String,
    pub token: String,
    pub prefix: String,
}

#[utoipa::path(
    post,
    path = "/admin/tokens",
    tag = "admin",
    request_body = CreateTokenRequest,
    responses(
        (status = 200, body = CreatedTokenResponse, description = "Token created - token is only shown once"),
        (status = 400, description = "Invalid input"),
    )
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

    let mut random = [0u8; 24];
    OsRng.fill_bytes(&mut random);
    let secret = URL_SAFE_NO_PAD.encode(random);
    let prefix: String = secret.chars().take(8).collect();
    let token = format!("stl_{prefix}_{secret}");

    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let token_hash = Argon2::default()
        .hash_password(token.as_bytes(), &salt)
        .map_err(|_| ApiError::internal("failed to hash token"))?
        .to_string();

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO api_tokens (id, name, prefix, token_hash, scope) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(input.name.trim())
    .bind(&prefix)
    .bind(token_hash)
    .bind(scope)
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

#[utoipa::path(
    get,
    path = "/admin/tokens",
    tag = "admin",
    responses(
        (status = 200, body = Vec<TokenResponse>),
    )
)]
pub async fn list_tokens(State(state): State<AppState>) -> Result<Json<Vec<TokenResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, name, scope, prefix, last_used_at, revoked_at, created_at FROM api_tokens ORDER BY created_at DESC",
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
            last_used_at: row.get("last_used_at"),
            revoked_at: row.get("revoked_at"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(items))
}

#[utoipa::path(
    delete,
    path = "/admin/tokens/{id}",
    tag = "admin",
    responses(
        (status = 200, description = "Token revoked"),
        (status = 404, description = "Token not found"),
    )
)]
pub async fn revoke_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query("UPDATE api_tokens SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("token not found"));
    }

    Ok(Json(serde_json::json!({"revoked": true, "id": id})))
}
