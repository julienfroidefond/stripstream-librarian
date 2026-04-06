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
