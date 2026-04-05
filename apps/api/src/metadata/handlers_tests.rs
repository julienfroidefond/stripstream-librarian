use super::*;
use serde_json::json;

// -----------------------------------------------------------------------
// is_field_locked
// -----------------------------------------------------------------------

#[test]
fn field_locked_when_true() {
    let locked = json!({"description": true, "authors": false});
    assert!(is_field_locked(&locked, "description"));
}

#[test]
fn field_not_locked_when_false() {
    let locked = json!({"description": false});
    assert!(!is_field_locked(&locked, "description"));
}

#[test]
fn field_not_locked_when_absent() {
    let locked = json!({});
    assert!(!is_field_locked(&locked, "description"));
}

#[test]
fn field_not_locked_when_null() {
    let locked = json!({"description": null});
    assert!(!is_field_locked(&locked, "description"));
}

#[test]
fn field_not_locked_when_non_boolean() {
    let locked = json!({"description": "true"});
    assert!(!is_field_locked(&locked, "description"));
}

// -----------------------------------------------------------------------
// classify_field_change
// -----------------------------------------------------------------------

#[test]
fn classify_returns_none_when_new_is_none() {
    let locked = json!({});
    let result = classify_field_change("title", Some(json!("old")), None, &locked);
    assert!(result.is_none());
}

#[test]
fn classify_returns_updated_when_values_differ() {
    let locked = json!({});
    let result = classify_field_change(
        "description",
        Some(json!("old desc")),
        Some(json!("new desc")),
        &locked,
    );
    let (is_skipped, change) = result.expect("should return Some");
    assert!(!is_skipped);
    assert_eq!(change.field, "description");
    assert_eq!(change.old_value, Some(json!("old desc")));
    assert_eq!(change.new_value, Some(json!("new desc")));
}

#[test]
fn classify_returns_skipped_when_locked() {
    let locked = json!({"description": true});
    let result = classify_field_change(
        "description",
        Some(json!("old")),
        Some(json!("new")),
        &locked,
    );
    let (is_skipped, change) = result.expect("should return Some");
    assert!(is_skipped);
    assert_eq!(change.field, "description");
}

#[test]
fn classify_returns_none_when_values_equal_and_unlocked() {
    let locked = json!({});
    let result = classify_field_change(
        "title",
        Some(json!("same")),
        Some(json!("same")),
        &locked,
    );
    assert!(result.is_none(), "identical values should produce no change");
}

#[test]
fn classify_updated_from_none_to_some() {
    let locked = json!({});
    let result = classify_field_change("isbn", None, Some(json!("978-123")), &locked);
    let (is_skipped, change) = result.expect("should return Some");
    assert!(!is_skipped);
    assert!(change.old_value.is_none());
    assert_eq!(change.new_value, Some(json!("978-123")));
}

#[test]
fn classify_locked_field_still_reported_even_when_old_is_none() {
    let locked = json!({"isbn": true});
    let result = classify_field_change("isbn", None, Some(json!("978-123")), &locked);
    let (is_skipped, _change) = result.expect("should return Some");
    assert!(is_skipped);
}

// -----------------------------------------------------------------------
// extract_string_array
// -----------------------------------------------------------------------

#[test]
fn extract_string_array_present() {
    let metadata = json!({"authors": ["Alice", "Bob"]});
    let result = extract_string_array(&metadata, "authors");
    assert_eq!(result, vec!["Alice", "Bob"]);
}

#[test]
fn extract_string_array_missing_key() {
    let metadata = json!({"title": "test"});
    let result = extract_string_array(&metadata, "authors");
    assert!(result.is_empty());
}

#[test]
fn extract_string_array_not_array() {
    let metadata = json!({"authors": "single author"});
    let result = extract_string_array(&metadata, "authors");
    assert!(result.is_empty());
}

#[test]
fn extract_string_array_filters_non_strings() {
    let metadata = json!({"authors": ["Alice", 42, null, "Bob"]});
    let result = extract_string_array(&metadata, "authors");
    assert_eq!(result, vec!["Alice", "Bob"]);
}

#[test]
fn extract_string_array_empty_array() {
    let metadata = json!({"authors": []});
    let result = extract_string_array(&metadata, "authors");
    assert!(result.is_empty());
}

// -----------------------------------------------------------------------
// FieldChange serialization
// -----------------------------------------------------------------------

#[test]
fn field_change_skips_none_values_in_json() {
    let change = FieldChange {
        field: "title".to_string(),
        old_value: None,
        new_value: Some(json!("new")),
    };
    let json = serde_json::to_value(&change).unwrap();
    assert!(!json.as_object().unwrap().contains_key("old_value"));
    assert!(json.as_object().unwrap().contains_key("new_value"));
}

#[test]
fn field_change_includes_both_when_present() {
    let change = FieldChange {
        field: "description".to_string(),
        old_value: Some(json!("old")),
        new_value: Some(json!("new")),
    };
    let json = serde_json::to_value(&change).unwrap();
    assert_eq!(json["field"], "description");
    assert_eq!(json["old_value"], "old");
    assert_eq!(json["new_value"], "new");
}

// -----------------------------------------------------------------------
// SyncReport / SeriesSyncReport defaults
// -----------------------------------------------------------------------

#[test]
fn sync_report_default_is_empty() {
    let report = SyncReport::default();
    assert!(report.series.is_none());
    assert!(report.books.is_empty());
    assert_eq!(report.books_matched, 0);
    assert_eq!(report.books_unmatched, 0);
    assert!(report.books_message.is_none());
}

#[test]
fn series_sync_report_default_is_empty() {
    let report = SeriesSyncReport::default();
    assert!(report.fields_updated.is_empty());
    assert!(report.fields_skipped.is_empty());
}

// -----------------------------------------------------------------------
// ApproveRequest deserialization
// -----------------------------------------------------------------------

#[test]
fn approve_request_defaults_to_false() {
    let json = r#"{}"#;
    let req: ApproveRequest = serde_json::from_str(json).unwrap();
    assert!(!req.sync_series);
    assert!(!req.sync_books);
}

#[test]
fn approve_request_with_both_true() {
    let json = r#"{"sync_series": true, "sync_books": true}"#;
    let req: ApproveRequest = serde_json::from_str(json).unwrap();
    assert!(req.sync_series);
    assert!(req.sync_books);
}

// -----------------------------------------------------------------------
// MetadataSearchRequest deserialization
// -----------------------------------------------------------------------

#[test]
fn search_request_requires_library_and_series() {
    let json = r#"{"library_id": "550e8400-e29b-41d4-a716-446655440000", "series_name": "Naruto"}"#;
    let req: MetadataSearchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.series_name, "Naruto");
    assert!(req.provider.is_none());
}

#[test]
fn search_request_with_provider_override() {
    let json = r#"{"library_id": "550e8400-e29b-41d4-a716-446655440000", "series_name": "test", "provider": "myanimelist"}"#;
    let req: MetadataSearchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.provider.as_deref(), Some("myanimelist"));
}

// -----------------------------------------------------------------------
// MetadataMatchRequest deserialization
// -----------------------------------------------------------------------

#[test]
fn match_request_deserializes_fully() {
    let json = r#"{
        "library_id": "550e8400-e29b-41d4-a716-446655440000",
        "series_name": "One Piece",
        "provider": "google_books",
        "external_id": "ext123",
        "title": "One Piece",
        "metadata_json": {"authors": ["Oda"]},
        "total_volumes": 105
    }"#;
    let req: MetadataMatchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.provider, "google_books");
    assert_eq!(req.total_volumes, Some(105));
    assert!(req.external_url.is_none());
}

// -----------------------------------------------------------------------
// ExternalMetadataLinkDto serialization
// -----------------------------------------------------------------------

#[test]
fn link_dto_serializes_optional_fields() {
    let dto = ExternalMetadataLinkDto {
        id: Uuid::nil(),
        library_id: Uuid::nil(),
        series_name: "test".to_string(),
        provider: "google_books".to_string(),
        external_id: "ext1".to_string(),
        external_url: None,
        status: "pending".to_string(),
        confidence: Some(0.95),
        metadata_json: json!({}),
        total_volumes_external: None,
        matched_at: "2024-01-01T00:00:00Z".to_string(),
        approved_at: None,
        synced_at: None,
    };
    let json = serde_json::to_value(&dto).unwrap();
    assert_eq!(json["status"], "pending");
    let confidence = json["confidence"].as_f64().unwrap();
    assert!((confidence - 0.95).abs() < 0.001, "confidence should be ~0.95, got {confidence}");
    assert!(json["external_url"].is_null());
}

// --- Missing books: total_volumes override ---

async fn setup_series_with_link(
    pool: &sqlx::PgPool,
    total_volumes: Option<i32>,
    local_book_count: i32,
    external_book_count: i32,
) -> (Uuid, Uuid, Uuid) {
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test', '/libraries/test')")
        .bind(lib_id).execute(pool).await.unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name, total_volumes) VALUES ($1, $2, 'Test', $3)")
        .bind(series_id).bind(lib_id).bind(total_volumes).execute(pool).await.unwrap();

    for i in 0..local_book_count {
        sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id) VALUES (gen_random_uuid(), $1, $2, 'comic', 'cbz', $3)")
            .bind(lib_id).bind(format!("Book {i}")).bind(series_id).execute(pool).await.unwrap();
    }

    let link_id = Uuid::new_v4();
    sqlx::query("INSERT INTO external_metadata_links (id, library_id, series_id, provider, external_id, status) VALUES ($1, $2, $3, 'test', 'ext:1', 'approved')")
        .bind(link_id).bind(lib_id).bind(series_id).execute(pool).await.unwrap();

    for i in 0..external_book_count {
        // Mark some as matched (book_id set) based on local_book_count
        let book_id: Option<Uuid> = if i < local_book_count {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM books WHERE series_id = $1 ORDER BY title LIMIT 1 OFFSET $2")
                .bind(series_id).bind(i as i64).fetch_optional(pool).await.unwrap()
        } else {
            None
        };
        sqlx::query("INSERT INTO external_book_metadata (id, link_id, external_book_id, title, volume_number, book_id) VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)")
            .bind(link_id).bind(format!("ext_{i}")).bind(format!("Vol {i}")).bind(i + 1).bind(book_id).execute(pool).await.unwrap();
    }

    (lib_id, series_id, link_id)
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_uses_series_total_volumes_override(pool: sqlx::PgPool) {
    // 3 local books, 5 external books from provider, but total_volumes overridden to 3
    let (_lib_id, _series_id, link_id) = setup_series_with_link(&pool, Some(3), 3, 5).await;

    let series_total: Option<i32> = sqlx::query_scalar("SELECT total_volumes FROM series WHERE id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let provider_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let local_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();

    let total_external = series_total.filter(|&v| v > 0).map(|v| v as i64).unwrap_or(provider_count);
    let missing = (total_external - local_count).max(0);

    assert_eq!(total_external, 3, "should use series.total_volumes override, not provider count");
    assert_eq!(missing, 0, "3 local / 3 total -> 0 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_falls_back_to_provider_count(pool: sqlx::PgPool) {
    // total_volumes is NULL -> use provider count (5)
    let (_lib_id, _series_id, link_id) = setup_series_with_link(&pool, None, 3, 5).await;

    let series_total: Option<i32> = sqlx::query_scalar("SELECT total_volumes FROM series WHERE id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let provider_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let local_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();

    let total_external = series_total.filter(|&v| v > 0).map(|v| v as i64).unwrap_or(provider_count);
    let missing = (total_external - local_count).max(0);

    assert_eq!(total_external, 5, "NULL total_volumes -> use provider count");
    assert_eq!(missing, 2, "3 local / 5 provider -> 2 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_not_negative(pool: sqlx::PgPool) {
    // Override total_volumes to 2 but have 3 local books
    let (_lib_id, _series_id, link_id) = setup_series_with_link(&pool, Some(2), 3, 5).await;

    let series_total: Option<i32> = sqlx::query_scalar("SELECT total_volumes FROM series WHERE id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let provider_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let local_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();

    let total_external = series_total.filter(|&v| v > 0).map(|v| v as i64).unwrap_or(provider_count);
    let missing = (total_external - local_count).max(0);

    assert_eq!(total_external, 2);
    assert_eq!(missing, 0, "2 total - 3 local capped at 0");
}
