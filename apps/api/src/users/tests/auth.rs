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

// LOCKED: RFC 7235 §2.1 defines the auth-scheme as case-insensitive, so
// "bearer" should be accepted. The test above locks the case-sensitive
// behaviour. See docs/KNOWN_ISSUES.md §1.
#[test]
fn bearer_token_lowercase_scheme_is_rejected() {
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
