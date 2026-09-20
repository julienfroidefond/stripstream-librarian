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
pub struct AuthUser {
    pub user_id: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub enum Scope {
    Admin,
    Read { user_id: uuid::Uuid },
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

    let as_user = req
        .headers()
        .get("X-As-User")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if let Some(user_id) = requested_impersonation(&state, as_user).await? {
        req.extensions_mut().insert(AuthUser { user_id });
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

    if let Scope::Read { user_id } = &scope {
        req.extensions_mut().insert(AuthUser { user_id: *user_id });
    } else if matches!(scope, Scope::Admin) {
        let as_user = req
            .headers()
            .get("X-As-User")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if let Some(user_id) = requested_impersonation(&state, as_user).await? {
            req.extensions_mut().insert(AuthUser { user_id });
        }
    }

    req.extensions_mut().insert(scope);
    Ok(next.run(req).await)
}

/// Resolves the `X-As-User` impersonation header, if present, to an existing user.
///
/// The header is only meaningful for admin callers (the backoffice sets it when
/// an operator is acting on behalf of a user), so it is rejected unless it names
/// a user that actually exists.
async fn requested_impersonation(
    state: &AppState,
    raw: Option<String>,
) -> Result<Option<uuid::Uuid>, ApiError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let user_id = uuid::Uuid::parse_str(&raw)
        .map_err(|_| ApiError::bad_request("X-As-User must be a valid user id"))?;

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;

    if !exists {
        return Err(ApiError::bad_request(
            "X-As-User references an unknown user",
        ));
    }

    Ok(Some(user_id))
}

fn bearer_token(req: &Request) -> Option<&str> {
    let value = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("Bearer").then_some(token)
}

/// Compares two secrets without short-circuiting on the first differing byte, so
/// the bootstrap token cannot be recovered by timing how many leading bytes a
/// guess got right. Only the length is compared directly, and token length is
/// not a secret.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn authenticate(state: &AppState, token: &str) -> Result<Scope, ApiError> {
    if constant_time_eq(token.as_bytes(), state.bootstrap_token.as_bytes()) {
        return Ok(Scope::Admin);
    }

    let prefix =
        parse_prefix(token).ok_or_else(|| ApiError::unauthorized("invalid token format"))?;

    let maybe_row = sqlx::query(
        r#"
        SELECT id, token_hash, scope, user_id FROM api_tokens
        WHERE prefix = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > NOW())
        "#,
    )
    .bind(prefix)
    .fetch_optional(&state.pool)
    .await?;

    let row = maybe_row.ok_or_else(|| ApiError::unauthorized("invalid token"))?;

    let token_hash: String = row
        .try_get("token_hash")
        .map_err(|_| ApiError::unauthorized("invalid token"))?;
    let parsed_hash =
        PasswordHash::new(&token_hash).map_err(|_| ApiError::unauthorized("invalid token"))?;

    Argon2::default()
        .verify_password(token.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::unauthorized("invalid token"))?;

    let token_id: uuid::Uuid = row
        .try_get("id")
        .map_err(|_| ApiError::unauthorized("invalid token"))?;
    sqlx::query("UPDATE api_tokens SET last_used_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(token_id)
        .execute(&state.pool)
        .await?;

    let scope: String = row
        .try_get("scope")
        .map_err(|_| ApiError::unauthorized("invalid token"))?;
    match scope.as_str() {
        "admin" => Ok(Scope::Admin),
        "read" => {
            let user_id: uuid::Uuid = row
                .try_get("user_id")
                .map_err(|_| ApiError::unauthorized("read token missing user_id"))?;
            Ok(Scope::Read { user_id })
        }
        _ => Err(ApiError::unauthorized("invalid token scope")),
    }
}

pub(crate) fn parse_prefix(token: &str) -> Option<&str> {
    // Format: stl_{8-char prefix}_{secret}
    // Base64 URL_SAFE peut contenir '_', donc on ne peut pas splitter aveuglément
    let rest = token.strip_prefix("stl_")?;
    if rest.len() < 10 {
        // 8 (prefix) + 1 ('_') + 1 (secret min)
        return None;
    }
    let prefix = &rest[..8];
    if rest.as_bytes().get(8) != Some(&b'_') {
        return None;
    }
    Some(prefix)
}

#[cfg(test)]
#[path = "tests/auth.rs"]
mod tests;
