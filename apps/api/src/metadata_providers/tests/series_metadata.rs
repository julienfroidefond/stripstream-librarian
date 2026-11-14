use serde_json::json;

use super::*;
use crate::metadata_providers::scoring::Score;

fn candidate(metadata_json: Value) -> SeriesCandidate {
    SeriesCandidate {
        external_id: "42".to_string(),
        title: "Akira".to_string(),
        authors: vec!["Katsuhiro Otomo".to_string()],
        description: Some("Neo-Tokyo".to_string()),
        publishers: vec!["Glénat".to_string()],
        start_year: Some(1982),
        total_volumes: Some(6),
        cover_url: Some("https://img/akira.jpg".to_string()),
        external_url: None,
        score: Score::exact(),
        metadata_json,
    }
}

#[test]
fn stored_metadata_json_adds_typed_fields_missing_from_the_blob() {
    // Shape of Google Books / OpenLibrary / ComicVine / AniList / SensCritique:
    // the typed fields are not in the blob.
    let stored =
        candidate(json!({ "status": "FINISHED", "genres": ["SF"] })).stored_metadata_json();

    assert_eq!(stored["authors"], json!(["Katsuhiro Otomo"]));
    assert_eq!(stored["publishers"], json!(["Glénat"]));
    assert_eq!(stored["start_year"], json!(1982));
    assert_eq!(stored["cover_url"], json!("https://img/akira.jpg"));
    assert_eq!(stored["description"], json!("Neo-Tokyo"));
    // Blob-only keys are kept.
    assert_eq!(stored["status"], json!("FINISHED"));
    assert_eq!(stored["genres"], json!(["SF"]));
}

#[test]
fn empty_typed_fields_never_erase_the_blob() {
    let mut c = candidate(json!({
        "authors": ["From blob"],
        "description": "Blob description",
        "start_year": 1990,
    }));
    c.authors = vec![];
    c.description = Some("  ".to_string());
    c.start_year = None;
    c.cover_url = None;

    let stored = c.stored_metadata_json();

    assert_eq!(stored["authors"], json!(["From blob"]));
    assert_eq!(stored["description"], json!("Blob description"));
    assert_eq!(stored["start_year"], json!(1990));
    assert!(stored.get("cover_url").is_none());
}

#[test]
fn non_object_blob_becomes_an_object() {
    let stored = enrich_series_metadata_json(
        &Value::Null,
        CandidateSeriesFields {
            start_year: Some(2001),
            ..Default::default()
        },
    );
    assert_eq!(stored, json!({ "start_year": 2001 }));
}

#[test]
fn merge_snapshot_keeps_fields_the_provider_no_longer_returns() {
    let stored = json!({
        "authors": ["Old author"],
        "publishers": ["Old publisher"],
        "start_year": 1982,
        "status": "FINISHED",
        "rating": 8.1,
    });
    let fresh = json!({
        "authors": [],
        "publishers": ["New publisher"],
        "status": null,
        "rating": 8.4,
        "genres": ["SF"],
    });

    let merged = merge_series_snapshot(&stored, &fresh);

    assert_eq!(merged["authors"], json!(["Old author"]));
    assert_eq!(merged["publishers"], json!(["New publisher"]));
    assert_eq!(merged["start_year"], json!(1982));
    assert_eq!(merged["status"], json!("FINISHED"));
    assert_eq!(merged["rating"], json!(8.4));
    assert_eq!(merged["genres"], json!(["SF"]));
}

#[test]
fn merge_snapshot_over_empty_stored_value_is_the_fresh_value() {
    let fresh = json!({ "status": "RELEASING", "authors": [] });
    assert_eq!(merge_series_snapshot(&Value::Null, &fresh), fresh);
}
