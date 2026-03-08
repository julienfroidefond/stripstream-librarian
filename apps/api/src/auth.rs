use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use sqlx::Row;

use crate::{error::ApiError, state::AppState};

#[derive(Clone, Debug)]
pub enum Scope {
    Admin,
    Read,
}

pub async fn require_admin(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = bearer_token(&req).ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
    let scope = authenticate(&state, token).await?;

    if !matches!(scope, Scope::Admin) {
        return Err(ApiError::forbidden("admin scope required"));
    }

    req.extensions_mut().insert(scope);
    Ok(next.run(req).await)
}

pub async fn require_read(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = bearer_token(&req).ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
    let scope = authenticate(&state, token).await?;

    req.extensions_mut().insert(scope);
    Ok(next.run(req).await)
}

fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

async fn authenticate(state: &AppState, token: &str) -> Result<Scope, ApiError> {
    if token == state.bootstrap_token.as_ref() {
        return Ok(Scope::Admin);
    }

    let prefix = parse_prefix(token).ok_or_else(|| ApiError::unauthorized("invalid token format"))?;

    let maybe_row = sqlx::query(
        r#"
        SELECT id, token_hash, scope
        FROM api_tokens
        WHERE prefix = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())
        "#,
    )
    .bind(prefix)
    .fetch_optional(&state.pool)
    .await?;

    let row = maybe_row.ok_or_else(|| ApiError::unauthorized("invalid token"))?;

    let token_hash: String = row.try_get("token_hash").map_err(|_| ApiError::unauthorized("invalid token"))?;
    let parsed_hash = PasswordHash::new(&token_hash).map_err(|_| ApiError::unauthorized("invalid token"))?;

    Argon2::default()
        .verify_password(token.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::unauthorized("invalid token"))?;

    let token_id: uuid::Uuid = row.try_get("id").map_err(|_| ApiError::unauthorized("invalid token"))?;
    sqlx::query("UPDATE api_tokens SET last_used_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(token_id)
        .execute(&state.pool)
        .await?;

    let scope: String = row.try_get("scope").map_err(|_| ApiError::unauthorized("invalid token"))?;
    match scope.as_str() {
        "admin" => Ok(Scope::Admin),
        "read" => Ok(Scope::Read),
        _ => Err(ApiError::unauthorized("invalid token scope")),
    }
}

fn parse_prefix(token: &str) -> Option<&str> {
    let mut parts = token.split('_');
    let namespace = parts.next()?;
    let prefix = parts.next()?;
    let secret = parts.next()?;
    if namespace != "stl" || secret.is_empty() || prefix.len() < 6 {
        return None;
    }
    Some(prefix)
}
