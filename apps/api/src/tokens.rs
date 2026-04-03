use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use axum::{extract::{Path, State}, Json};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use utoipa::ToSchema;

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
        return Err(ApiError::bad_request("user_id is required for read-scoped tokens"));
    }

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
pub async fn list_tokens(State(state): State<AppState>) -> Result<Json<Vec<TokenResponse>>, ApiError> {
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
    let result = sqlx::query("UPDATE api_tokens SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL")
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
pub(crate) fn build_token(secret_bytes: &[u8]) -> (String, String) {
    let secret = URL_SAFE_NO_PAD.encode(secret_bytes);
    let prefix: String = secret.chars().take(8).collect();
    let token = format!("stl_{prefix}_{secret}");
    (token, prefix)
}

/// Validate a scope string: must be "admin" or "read".
pub(crate) fn validate_scope(scope: Option<&str>) -> Result<&'static str, &'static str> {
    match scope.unwrap_or("read") {
        "admin" => Ok("admin"),
        "read" => Ok("read"),
        _ => Err("scope must be 'admin' or 'read'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::PasswordVerifier;

    // -----------------------------------------------------------------------
    // Token format: build_token
    // -----------------------------------------------------------------------

    #[test]
    fn build_token_has_stl_prefix() {
        let bytes = [0u8; 24];
        let (token, _prefix) = build_token(&bytes);
        assert!(token.starts_with("stl_"), "token should start with stl_");
    }

    #[test]
    fn build_token_prefix_is_8_chars() {
        let bytes = [42u8; 24];
        let (_token, prefix) = build_token(&bytes);
        assert_eq!(prefix.len(), 8);
    }

    #[test]
    fn build_token_format_matches_parse_prefix() {
        // A generated token must be parseable by auth::parse_prefix
        let bytes = [0xAB; 24];
        let (token, expected_prefix) = build_token(&bytes);
        let parsed = crate::auth::parse_prefix(&token);
        assert_eq!(parsed, Some(expected_prefix.as_str()));
    }

    #[test]
    fn build_token_different_secrets_produce_different_tokens() {
        let (token1, _) = build_token(&[1u8; 24]);
        let (token2, _) = build_token(&[2u8; 24]);
        assert_ne!(token1, token2);
    }

    #[test]
    fn build_token_prefix_matches_start_of_secret() {
        let bytes = [0xFF; 24];
        let (token, prefix) = build_token(&bytes);
        // Token format: stl_{prefix}_{secret}
        // The prefix should be the first 8 chars of the base64 secret
        let secret = URL_SAFE_NO_PAD.encode(bytes);
        let expected_prefix: String = secret.chars().take(8).collect();
        assert_eq!(prefix, expected_prefix);
        assert!(token.contains(&format!("stl_{prefix}_")));
    }

    #[test]
    fn build_token_round_trip_with_real_random_bytes() {
        let mut bytes = [0u8; 24];
        OsRng.fill_bytes(&mut bytes);
        let (token, prefix) = build_token(&bytes);

        // Verify the format
        assert!(token.starts_with("stl_"));
        assert_eq!(prefix.len(), 8);
        assert!(token.len() > 13); // "stl_" + 8 + "_" + at least 1

        // Verify parse_prefix can extract it
        let parsed = crate::auth::parse_prefix(&token);
        assert_eq!(parsed, Some(prefix.as_str()));
    }

    // -----------------------------------------------------------------------
    // Token hash: argon2 round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn token_hash_verifies_correctly() {
        let (token, _) = build_token(&[0xDE; 24]);
        let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = Argon2::default()
            .hash_password(token.as_bytes(), &salt)
            .expect("hash should succeed")
            .to_string();

        let parsed = argon2::PasswordHash::new(&hash).expect("parse hash");
        let result = Argon2::default().verify_password(token.as_bytes(), &parsed);
        assert!(result.is_ok(), "correct token should verify");
    }

    #[test]
    fn token_hash_rejects_wrong_token() {
        let (token, _) = build_token(&[0xDE; 24]);
        let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = Argon2::default()
            .hash_password(token.as_bytes(), &salt)
            .expect("hash should succeed")
            .to_string();

        let parsed = argon2::PasswordHash::new(&hash).expect("parse hash");
        let wrong_token = "stl_XXXXXXXX_totally_wrong";
        let result = Argon2::default().verify_password(wrong_token.as_bytes(), &parsed);
        assert!(result.is_err(), "wrong token should not verify");
    }

    // -----------------------------------------------------------------------
    // validate_scope
    // -----------------------------------------------------------------------

    #[test]
    fn validate_scope_defaults_to_read() {
        assert_eq!(validate_scope(None), Ok("read"));
    }

    #[test]
    fn validate_scope_accepts_read() {
        assert_eq!(validate_scope(Some("read")), Ok("read"));
    }

    #[test]
    fn validate_scope_accepts_admin() {
        assert_eq!(validate_scope(Some("admin")), Ok("admin"));
    }

    #[test]
    fn validate_scope_rejects_unknown() {
        assert!(validate_scope(Some("superuser")).is_err());
    }

    #[test]
    fn validate_scope_rejects_empty() {
        assert!(validate_scope(Some("")).is_err());
    }

    #[test]
    fn validate_scope_is_case_sensitive() {
        assert!(validate_scope(Some("Admin")).is_err());
        assert!(validate_scope(Some("READ")).is_err());
    }

    // -----------------------------------------------------------------------
    // CreateTokenRequest validation (deserialization)
    // -----------------------------------------------------------------------

    #[test]
    fn create_token_request_deserializes_with_all_fields() {
        let json = r#"{"name": "test", "scope": "admin", "user_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: CreateTokenRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "test");
        assert_eq!(req.scope.as_deref(), Some("admin"));
        assert!(req.user_id.is_some());
    }

    #[test]
    fn create_token_request_deserializes_minimal() {
        let json = r#"{"name": "test"}"#;
        let req: CreateTokenRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "test");
        assert!(req.scope.is_none());
        assert!(req.user_id.is_none());
    }

    #[test]
    fn create_token_request_empty_name_is_deserializable() {
        // The handler checks name.trim().is_empty(), not the deserializer
        let json = r#"{"name": "   "}"#;
        let req: CreateTokenRequest = serde_json::from_str(json).unwrap();
        assert!(req.name.trim().is_empty());
    }

    // -----------------------------------------------------------------------
    // CreatedTokenResponse serialization
    // -----------------------------------------------------------------------

    #[test]
    fn created_token_response_serializes() {
        let resp = CreatedTokenResponse {
            id: Uuid::nil(),
            name: "test".to_string(),
            scope: "read".to_string(),
            token: "stl_abcdefgh_secret".to_string(),
            prefix: "abcdefgh".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["name"], "test");
        assert_eq!(json["scope"], "read");
        assert!(json["token"].as_str().unwrap().starts_with("stl_"));
    }
}
