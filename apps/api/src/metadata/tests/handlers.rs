use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};

use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};

use super::*;
use crate::state::{
    DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};
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
    let locked = json!({"description": "yes"});
    assert!(!is_field_locked(&locked, "description"));
}

#[test]
fn field_locked_when_string_true() {
    let locked = json!({"description": "true"});
    assert!(is_field_locked(&locked, "description"));
}

#[test]
fn field_locked_when_string_true_uppercase() {
    let locked = json!({"description": "TRUE"});
    assert!(is_field_locked(&locked, "description"));
}

#[test]
fn field_locked_when_nonzero_number() {
    let locked = json!({"description": 1});
    assert!(is_field_locked(&locked, "description"));
}

#[test]
fn field_not_locked_when_zero_number() {
    let locked = json!({"description": 0});
    assert!(!is_field_locked(&locked, "description"));
}

// -----------------------------------------------------------------------
// classify_field_change
// -----------------------------------------------------------------------

#[test]
fn classify_returns_cleared_when_new_is_none_and_old_is_some() {
    let locked = json!({});
    let result = classify_field_change("title", Some(json!("old")), None, &locked);
    let (is_skipped, change) = result.expect("should return Some");
    assert!(!is_skipped);
    assert_eq!(change.field, "title");
    assert_eq!(change.old_value, Some(json!("old")));
    assert_eq!(change.new_value, None);
}

#[test]
fn classify_returns_none_when_both_values_are_none() {
    let locked = json!({});
    let result = classify_field_change("title", None, None, &locked);
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
    let result = classify_field_change("title", Some(json!("same")), Some(json!("same")), &locked);
    assert!(
        result.is_none(),
        "identical values should produce no change"
    );
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
        is_primary: false,
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
    assert!(
        (confidence - 0.95).abs() < 0.001,
        "confidence should be ~0.95, got {confidence}"
    );
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
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test', '/libraries/test')",
    )
    .bind(lib_id)
    .execute(pool)
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO series (id, library_id, name, total_volumes) VALUES ($1, $2, 'Test', $3)",
    )
    .bind(series_id)
    .bind(lib_id)
    .bind(total_volumes)
    .execute(pool)
    .await
    .unwrap();

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
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM books WHERE series_id = $1 ORDER BY title LIMIT 1 OFFSET $2",
            )
            .bind(series_id)
            .bind(i as i64)
            .fetch_optional(pool)
            .await
            .unwrap()
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
    let provider_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
            .bind(link_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let local_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();

    let total_external = series_total
        .filter(|&v| v > 0)
        .map(|v| v as i64)
        .unwrap_or(provider_count);
    let missing = (total_external - local_count).max(0);

    assert_eq!(
        total_external, 3,
        "should use series.total_volumes override, not provider count"
    );
    assert_eq!(missing, 0, "3 local / 3 total -> 0 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_falls_back_to_provider_count(pool: sqlx::PgPool) {
    // total_volumes is NULL -> use provider count (5)
    let (_lib_id, _series_id, link_id) = setup_series_with_link(&pool, None, 3, 5).await;

    let series_total: Option<i32> = sqlx::query_scalar("SELECT total_volumes FROM series WHERE id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let provider_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
            .bind(link_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let local_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();

    let total_external = series_total
        .filter(|&v| v > 0)
        .map(|v| v as i64)
        .unwrap_or(provider_count);
    let missing = (total_external - local_count).max(0);

    assert_eq!(
        total_external, 5,
        "NULL total_volumes -> use provider count"
    );
    assert_eq!(missing, 2, "3 local / 5 provider -> 2 missing");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_not_negative(pool: sqlx::PgPool) {
    // Override total_volumes to 2 but have 3 local books
    let (_lib_id, _series_id, link_id) = setup_series_with_link(&pool, Some(2), 3, 5).await;

    let series_total: Option<i32> = sqlx::query_scalar("SELECT total_volumes FROM series WHERE id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();
    let provider_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_book_metadata WHERE link_id = $1")
            .bind(link_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let local_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = (SELECT series_id FROM external_metadata_links WHERE id = $1)")
        .bind(link_id).fetch_one(&pool).await.unwrap();

    let total_external = series_total
        .filter(|&v| v > 0)
        .map(|v| v as i64)
        .unwrap_or(provider_count);
    let missing = (total_external - local_count).max(0);

    assert_eq!(total_external, 2);
    assert_eq!(missing, 0, "2 total - 3 local capped at 0");
}

// --- Missing books: volume_type filtering ---

async fn setup_series_with_link_and_hs(
    pool: &sqlx::PgPool,
    total_volumes: Option<i32>,
    regular_count: i32,
    hs_count: i32,
    external_book_count: i32,
) -> (Uuid, Uuid, Uuid) {
    let lib_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'test_hs', '/libraries/test_hs')",
    )
    .bind(lib_id)
    .execute(pool)
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO series (id, library_id, name, total_volumes) VALUES ($1, $2, 'TestHS', $3)",
    )
    .bind(series_id)
    .bind(lib_id)
    .bind(total_volumes)
    .execute(pool)
    .await
    .unwrap();

    for i in 0..regular_count {
        sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id, volume, volume_type) VALUES (gen_random_uuid(), $1, $2, 'comic', 'cbz', $3, $4, 'regular')")
            .bind(lib_id).bind(format!("Vol {}", i + 1)).bind(series_id).bind(i + 1).execute(pool).await.unwrap();
    }
    for i in 0..hs_count {
        sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id, volume, volume_type) VALUES (gen_random_uuid(), $1, $2, 'comic', 'cbz', $3, $4, 'hs')")
            .bind(lib_id).bind(format!("HS {}", i + 1)).bind(series_id).bind(i + 1).execute(pool).await.unwrap();
    }

    let link_id = Uuid::new_v4();
    sqlx::query("INSERT INTO external_metadata_links (id, library_id, series_id, provider, external_id, status) VALUES ($1, $2, $3, 'test', 'ext:1', 'approved')")
        .bind(link_id).bind(lib_id).bind(series_id).execute(pool).await.unwrap();

    for i in 0..external_book_count {
        let book_id: Option<Uuid> = if i < regular_count {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM books WHERE series_id = $1 AND volume_type IN ('regular', 'integral') ORDER BY title LIMIT 1 OFFSET $2",
            )
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
async fn missing_books_total_local_excludes_hs(pool: sqlx::PgPool) {
    // 3 regular + 2 HS = 5 books, but total_local should be 3
    let (_lib_id, series_id, _link_id) =
        setup_series_with_link_and_hs(&pool, Some(5), 3, 2, 5).await;

    let total_local: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books WHERE series_id = $1 AND volume_type IN ('regular', 'integral')",
    )
    .bind(series_id).fetch_one(&pool).await.unwrap();

    let total_all: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = $1")
        .bind(series_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(total_all, 5, "total books including HS");
    assert_eq!(
        total_local, 3,
        "total_local should only count regular books"
    );

    let series_total: Option<i32> =
        sqlx::query_scalar("SELECT total_volumes FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let total_external = series_total
        .filter(|&v| v > 0)
        .map(|v| v as i64)
        .unwrap_or(5);
    let missing = (total_external - total_local).max(0);
    assert_eq!(missing, 2, "5 total - 3 regular = 2 missing (HS excluded)");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_hs_volume_not_matched_to_regular(pool: sqlx::PgPool) {
    // Scenario: HS vol=1 should NOT match external book vol=1
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'hs_nomatch', '/libraries/hs_nomatch')")
        .bind(lib_id).execute(&pool).await.unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'NoMatch')")
        .bind(series_id)
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    // Only an HS book with volume=1, no regular book
    sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id, volume, volume_type) VALUES (gen_random_uuid(), $1, 'HS 1', 'comic', 'cbz', $2, 1, 'hs')")
        .bind(lib_id).bind(series_id).execute(&pool).await.unwrap();

    let link_id = Uuid::new_v4();
    sqlx::query("INSERT INTO external_metadata_links (id, library_id, series_id, provider, external_id, status) VALUES ($1, $2, $3, 'test', 'ext:1', 'approved')")
        .bind(link_id).bind(lib_id).bind(series_id).execute(&pool).await.unwrap();

    // External book with volume_number=1 (should NOT match the HS book)
    let ebm_id = Uuid::new_v4();
    sqlx::query("INSERT INTO external_book_metadata (id, link_id, external_book_id, title, volume_number, book_id) VALUES ($1, $2, 'ext_1', 'Vol 1', 1, NULL)")
        .bind(ebm_id).bind(link_id).execute(&pool).await.unwrap();

    // Run rematch logic (same as rematch_unlinked_books)
    let result = sqlx::query(
        r#"
        UPDATE external_book_metadata ebm
        SET book_id = matched.book_id
        FROM (
            SELECT DISTINCT ON (ebm2.id)
                ebm2.id AS ebm_id,
                b.id AS book_id
            FROM external_book_metadata ebm2
            JOIN external_metadata_links eml ON eml.id = ebm2.link_id
            JOIN books b ON b.library_id = eml.library_id
                AND b.series_id = eml.series_id
                AND b.volume = ebm2.volume_number
                AND b.volume_type IN ('regular', 'integral')
            WHERE eml.library_id = $1
              AND ebm2.book_id IS NULL
              AND ebm2.volume_number IS NOT NULL
              AND eml.status = 'approved'
        ) matched
        WHERE ebm.id = matched.ebm_id
        "#,
    )
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        result.rows_affected(),
        0,
        "HS book volume=1 should NOT match external volume=1"
    );

    // Verify external book is still unmatched
    let book_id: Option<Uuid> =
        sqlx::query_scalar("SELECT book_id FROM external_book_metadata WHERE id = $1")
            .bind(ebm_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(book_id.is_none(), "external book should remain unmatched");
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn missing_books_integral_makes_series_complete(pool: sqlx::PgPool) {
    // Series with total_volumes=3, only an intégrale → should be 3/3, 0 missing
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'int_test', '/libraries/int_test')")
        .bind(lib_id).execute(&pool).await.unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO series (id, library_id, name, total_volumes) VALUES ($1, $2, 'INT Series', 3)",
    )
    .bind(series_id)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    // One integral book
    sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id, volume_type) VALUES (gen_random_uuid(), $1, 'INT', 'comic', 'cbz', $2, 'integral')")
        .bind(lib_id).bind(series_id).execute(&pool).await.unwrap();

    let link_id = Uuid::new_v4();
    sqlx::query("INSERT INTO external_metadata_links (id, library_id, series_id, provider, external_id, status) VALUES ($1, $2, $3, 'test', 'ext:1', 'approved')")
        .bind(link_id).bind(lib_id).bind(series_id).execute(&pool).await.unwrap();

    // 3 external books, none matched
    for i in 1..=3 {
        sqlx::query("INSERT INTO external_book_metadata (id, link_id, external_book_id, title, volume_number, book_id) VALUES (gen_random_uuid(), $1, $2, $3, $4, NULL)")
            .bind(link_id).bind(format!("ext_{i}")).bind(format!("Vol {i}")).bind(i).execute(&pool).await.unwrap();
    }

    // Check: has_integral should force total_local = total_external
    let has_integral: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM books WHERE series_id = $1 AND volume_type = 'integral')",
    )
    .bind(series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(has_integral, "series should have an integral book");

    let total_local: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM books WHERE series_id = $1 AND volume_type IN ('regular', 'integral')",
    )
    .bind(series_id).fetch_one(&pool).await.unwrap();
    assert_eq!(total_local, 1, "raw count is 1 (the integral)");

    let series_total: Option<i32> =
        sqlx::query_scalar("SELECT total_volumes FROM series WHERE id = $1")
            .bind(series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let total_external = series_total
        .filter(|&v| v > 0)
        .map(|v| v as i64)
        .unwrap_or(3);

    // With integral: effective local = total_external, missing = 0
    let effective_local = if has_integral {
        total_external
    } else {
        total_local
    };
    let missing = if has_integral {
        0
    } else {
        (total_external - total_local).max(0)
    };

    assert_eq!(
        effective_local, 3,
        "integral → effective local = total_external"
    );
    assert_eq!(missing, 0, "integral → 0 missing");
}

// -----------------------------------------------------------------------
// Primary provider selection (handler level)
// -----------------------------------------------------------------------

fn test_state(pool: PgPool) -> AppState {
    AppState {
        pool,
        bootstrap_token: Arc::from("test-token"),
        page_cache: Arc::new(Mutex::new(crate::state::PageCache::new(1))),
        disk_cache_stats: Arc::new(Mutex::new(None::<DiskCacheStatsSnapshot>)),
        page_render_locks: Arc::new(PageRenderLocks::new(1)),
        page_render_limit: Arc::new(Semaphore::new(1)),
        metrics: Arc::new(Metrics {
            requests_total: AtomicU64::new(0),
            page_cache_hits: AtomicU64::new(0),
            page_cache_misses: AtomicU64::new(0),
        }),
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit::new())),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
    }
}

async fn seed_series_with_links(pool: &PgPool, providers: &[&str]) -> (Uuid, Uuid, Vec<Uuid>) {
    let library_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'primary', '/libraries/primary')",
    )
    .bind(library_id)
    .execute(pool)
    .await
    .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Primary Series')")
        .bind(series_id)
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();

    let mut links = Vec::new();
    for (index, provider) in providers.iter().enumerate() {
        let link_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO external_metadata_links \
                 (id, library_id, series_id, provider, external_id, status, metadata_json) \
             VALUES ($1, $2, $3, $4, $5, 'pending', '{}'::jsonb)",
        )
        .bind(link_id)
        .bind(library_id)
        .bind(series_id)
        .bind(provider)
        .bind(format!("ext:{index}"))
        .execute(pool)
        .await
        .unwrap();
        links.push(link_id);
    }

    (library_id, series_id, links)
}

async fn link_is_primary(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT is_primary FROM external_metadata_links WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn link_status(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM external_metadata_links WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn approve(state: &AppState, id: Uuid, is_primary: Option<bool>) {
    let _ = approve_metadata(
        State(state.clone()),
        AxumPath(id),
        Json(ApproveRequest {
            sync_series: false,
            sync_books: false,
            is_primary,
        }),
    )
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_first_link_becomes_primary(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_lib, _series, links) = seed_series_with_links(&pool, &["provider_a"]).await;

    approve(&state, links[0], None).await;

    assert!(link_is_primary(&pool, links[0]).await);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_second_provider_keeps_first_primary(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_lib, _series, links) = seed_series_with_links(&pool, &["provider_a", "provider_b"]).await;

    approve(&state, links[0], None).await;
    approve(&state, links[1], None).await;

    assert!(
        link_is_primary(&pool, links[0]).await,
        "first approved link stays primary"
    );
    assert!(
        !link_is_primary(&pool, links[1]).await,
        "second approved link is a fallback"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn approve_with_force_switches_primary(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_lib, _series, links) = seed_series_with_links(&pool, &["provider_a", "provider_b"]).await;

    approve(&state, links[0], None).await;
    approve(&state, links[1], Some(true)).await;

    assert!(
        !link_is_primary(&pool, links[0]).await,
        "old primary cleared"
    );
    assert!(
        link_is_primary(&pool, links[1]).await,
        "forced primary wins"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reject_primary_promotes_oldest_approved(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_lib, _series, links) = seed_series_with_links(&pool, &["provider_a", "provider_b"]).await;

    approve(&state, links[0], None).await;
    approve(&state, links[1], None).await;

    let _ = reject_metadata(State(state.clone()), AxumPath(links[0]))
        .await
        .unwrap();

    assert_eq!(link_status(&pool, links[0]).await, "rejected");
    assert!(
        link_is_primary(&pool, links[1]).await,
        "oldest approved link promoted after rejection"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn delete_primary_promotes_oldest_approved(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_lib, _series, links) = seed_series_with_links(&pool, &["provider_a", "provider_b"]).await;

    approve(&state, links[0], None).await;
    approve(&state, links[1], None).await;

    let _ = delete_metadata_link(State(state.clone()), AxumPath(links[0]))
        .await
        .unwrap();

    assert!(
        link_is_primary(&pool, links[1]).await,
        "remaining approved link promoted after deletion"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn patch_promotes_link_to_primary(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_lib, _series, links) = seed_series_with_links(&pool, &["provider_a", "provider_b"]).await;

    approve(&state, links[0], None).await;
    approve(&state, links[1], None).await;

    let response = patch_metadata_link(
        State(state.clone()),
        AxumPath(links[1]),
        Json(PatchLinkRequest {
            is_primary: Some(true),
            sync_series: false,
            sync_books: false,
        }),
    )
    .await
    .unwrap();

    assert!(
        response.0.link.is_primary,
        "response reports the new primary"
    );
    assert!(
        !link_is_primary(&pool, links[0]).await,
        "previous primary cleared"
    );
    assert!(link_is_primary(&pool, links[1]).await);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn patch_rejects_non_approved_link(pool: PgPool) {
    let state = test_state(pool.clone());
    let (_lib, _series, links) = seed_series_with_links(&pool, &["provider_a"]).await;

    let error = match patch_metadata_link(
        State(state.clone()),
        AxumPath(links[0]),
        Json(PatchLinkRequest {
            is_primary: Some(true),
            sync_series: false,
            sync_books: false,
        }),
    )
    .await
    {
        Ok(_) => panic!("expected non-approved link to be rejected"),
        Err(error) => error,
    };

    assert_eq!(error.status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn reject_unknown_link_returns_not_found(pool: PgPool) {
    let state = test_state(pool.clone());
    let error = reject_metadata(State(state.clone()), AxumPath(Uuid::new_v4()))
        .await
        .unwrap_err();

    assert_eq!(error.status, axum::http::StatusCode::NOT_FOUND);
}
