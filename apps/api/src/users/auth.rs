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
        // Admin peut s'impersonifier via le header X-As-User
        if let Some(as_user_id) = req
            .headers()
            .get("X-As-User")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| uuid::Uuid::parse_str(v).ok())
        {
            req.extensions_mut().insert(AuthUser { user_id: as_user_id });
        }
    }

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
        SELECT id, token_hash, scope, user_id FROM api_tokens
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
mod tests {
    use super::*;
    use axum::http::{header::AUTHORIZATION, Request as HttpRequest};

    // -----------------------------------------------------------------------
    // parse_prefix
    // -----------------------------------------------------------------------

    #[test]
    fn parse_prefix_valid_token() {
        let token = "stl_abcdefgh_somesecretvalue";
        assert_eq!(parse_prefix(token), Some("abcdefgh"));
    }

    #[test]
    fn parse_prefix_exact_minimum_length() {
        // 8-char prefix + '_' + 1-char secret = 10 chars after "stl_"
        let token = "stl_12345678_x";
        assert_eq!(parse_prefix(token), Some("12345678"));
    }

    #[test]
    fn parse_prefix_missing_stl_prefix() {
        assert_eq!(parse_prefix("abc_12345678_secret"), None);
    }

    #[test]
    fn parse_prefix_empty_string() {
        assert_eq!(parse_prefix(""), None);
    }

    #[test]
    fn parse_prefix_only_stl_prefix() {
        assert_eq!(parse_prefix("stl_"), None);
    }

    #[test]
    fn parse_prefix_too_short_after_stl() {
        // 9 chars after "stl_" — needs at least 10
        assert_eq!(parse_prefix("stl_12345678"), None);
    }

    #[test]
    fn parse_prefix_no_separator_after_prefix() {
        // 8 chars + a non-underscore char at position 8
        assert_eq!(parse_prefix("stl_12345678Xsecret"), None);
    }

    #[test]
    fn parse_prefix_with_underscores_in_secret() {
        // Base64 URL_SAFE can contain underscores — prefix should still be first 8 chars
        let token = "stl_ABCD_fgh_secret_with_underscores";
        assert_eq!(parse_prefix(token), Some("ABCD_fgh"));
    }

    #[test]
    fn parse_prefix_long_secret() {
        let token = "stl_PREFIXab_aVeryLongSecretValueThatCouldBeBase64Encoded";
        assert_eq!(parse_prefix(token), Some("PREFIXab"));
    }

    // -----------------------------------------------------------------------
    // bearer_token
    // -----------------------------------------------------------------------

    #[test]
    fn bearer_token_extracts_correctly() {
        let req = HttpRequest::builder()
            .header(AUTHORIZATION, "Bearer my_token_here")
            .body(())
            .unwrap();
        // Convert to axum Request type
        let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
        assert_eq!(bearer_token(&req), Some("my_token_here"));
    }

    #[test]
    fn bearer_token_missing_header() {
        let req = HttpRequest::builder().body(()).unwrap();
        let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
        assert_eq!(bearer_token(&req), None);
    }

    #[test]
    fn bearer_token_wrong_scheme() {
        let req = HttpRequest::builder()
            .header(AUTHORIZATION, "Basic abc123")
            .body(())
            .unwrap();
        let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
        assert_eq!(bearer_token(&req), None);
    }

    #[test]
    fn bearer_token_case_sensitive_bearer() {
        // "bearer " (lowercase) should NOT match — the spec says "Bearer"
        let req = HttpRequest::builder()
            .header(AUTHORIZATION, "bearer my_token")
            .body(())
            .unwrap();
        let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
        assert_eq!(bearer_token(&req), None);
    }

    #[test]
    fn bearer_token_empty_token_value() {
        let req = HttpRequest::builder()
            .header(AUTHORIZATION, "Bearer ")
            .body(())
            .unwrap();
        let req = Request::from_parts(req.into_parts().0, axum::body::Body::empty());
        assert_eq!(bearer_token(&req), Some(""));
    }

    // -----------------------------------------------------------------------
    // Scope enum
    // -----------------------------------------------------------------------

    #[test]
    fn scope_admin_matches_correctly() {
        let scope = Scope::Admin;
        assert!(matches!(scope, Scope::Admin));
    }

    #[test]
    fn scope_read_does_not_match_admin() {
        let scope = Scope::Read {
            user_id: uuid::Uuid::nil(),
        };
        assert!(!matches!(scope, Scope::Admin));
    }

    #[test]
    fn scope_read_carries_user_id() {
        let id = uuid::Uuid::new_v4();
        let scope = Scope::Read { user_id: id };
        if let Scope::Read { user_id } = scope {
            assert_eq!(user_id, id);
        } else {
            panic!("expected Scope::Read");
        }
    }
}
