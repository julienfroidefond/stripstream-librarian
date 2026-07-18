use super::*;

#[test]
fn report_dto_serializes() {
    let dto = MetadataRefreshReportDto {
        job_id: Uuid::nil(),
        status: "success".to_string(),
        total_links: 10,
        refreshed: 3,
        unchanged: 6,
        errors: 1,
        changes: serde_json::json!([]),
    };
    let json = serde_json::to_value(&dto).unwrap();
    assert_eq!(json["status"], "success");
    assert_eq!(json["total_links"], 10);
    assert_eq!(json["refreshed"], 3);
    assert_eq!(json["unchanged"], 6);
    assert_eq!(json["errors"], 1);
    assert_eq!(json["changes"], serde_json::json!([]));
    assert_eq!(json["job_id"], Uuid::nil().to_string());
}

#[test]
fn series_refresh_result_serializes_updated() {
    let result = SeriesRefreshResult {
        series_name: "Blacksad".to_string(),
        provider: "google_books".to_string(),
        status: "updated".to_string(),
        series_changes: vec![FieldDiff {
            field: "title".to_string(),
            old: Some(serde_json::json!("Blacksad old")),
            new: Some(serde_json::json!("Blacksad")),
        }],
        book_changes: vec![],
        error: None,
    };
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["series_name"], "Blacksad");
    assert_eq!(json["status"], "updated");
    assert_eq!(json["series_changes"][0]["field"], "title");
    // error field should be absent (skip_serializing_if = None)
    assert!(json.get("error").is_none());
}

#[test]
fn series_refresh_result_serializes_error() {
    let result = SeriesRefreshResult {
        series_name: "Test".to_string(),
        provider: "anilist".to_string(),
        status: "error".to_string(),
        series_changes: vec![],
        book_changes: vec![],
        error: Some("provider timeout".to_string()),
    };
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"], "provider timeout");
}

#[test]
fn field_diff_skips_none_values() {
    let diff = FieldDiff {
        field: "summary".to_string(),
        old: None,
        new: Some(serde_json::json!("A new summary")),
    };
    let json = serde_json::to_value(&diff).unwrap();
    assert!(json.get("old").is_none());
    assert_eq!(json["new"], "A new summary");
}

#[test]
fn book_diff_serializes() {
    let diff = BookDiff {
        book_id: "abc-123".to_string(),
        title: "Volume 1".to_string(),
        volume: Some(1),
        changes: vec![FieldDiff {
            field: "page_count".to_string(),
            old: Some(serde_json::json!(100)),
            new: Some(serde_json::json!(120)),
        }],
    };
    let json = serde_json::to_value(&diff).unwrap();
    assert_eq!(json["book_id"], "abc-123");
    assert_eq!(json["volume"], 1);
    assert_eq!(json["changes"][0]["old"], 100);
    assert_eq!(json["changes"][0]["new"], 120);
}

#[test]
fn request_dto_deserializes_without_library_id() {
    let json = serde_json::json!({});
    let req: MetadataRefreshRequest = serde_json::from_value(json).unwrap();
    assert!(req.library_id.is_none());
}

#[test]
fn request_dto_deserializes_with_library_id() {
    let json = serde_json::json!({"library_id": "abc-123"});
    let req: MetadataRefreshRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.library_id.unwrap(), "abc-123");
}

#[test]
fn notification_change_shows_old_and_new_values() {
    let change = FieldDiff {
        field: "authors".to_string(),
        old: Some(serde_json::json!(["Old author"])),
        new: Some(serde_json::json!(["New author"])),
    };

    assert_eq!(
        format_notification_change(&change),
        "authors: [\"Old author\"] → [\"New author\"]"
    );
}

#[test]
fn notification_change_marks_missing_old_value() {
    let change = FieldDiff {
        field: "start_year".to_string(),
        old: None,
        new: Some(serde_json::json!(2026)),
    };

    assert_eq!(format_notification_change(&change), "start_year: ∅ → 2026");
}
