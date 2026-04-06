use super::*;

fn sorted(mut v: Vec<i32>) -> Vec<i32> {
    v.sort_unstable();
    v
}

#[test]
fn integral_french_accent() {
    assert!(is_integral_release("One Piece - Intégrale [CBZ]"));
    assert!(is_integral_release("Naruto Integrale FR"));
}

#[test]
fn integral_complet() {
    assert!(is_integral_release("Dragon Ball Complet [PDF]"));
    assert!(is_integral_release("Bleach Complete Edition"));
}

#[test]
fn integral_not_false_positive() {
    assert!(!is_integral_release("One Piece T05"));
    assert!(!is_integral_release("Naruto Tome 12"));
    assert!(!is_integral_release("Les Géants - 07 - Moon.cbz"));
    // "intégr" alone is not enough
    assert!(!is_integral_release("Naruto integration test"));
}

#[test]
fn integral_case_insensitive() {
    assert!(is_integral_release("INTEGRALE"));
    assert!(is_integral_release("COMPLET"));
    assert!(is_integral_release("Intégrale"));
}

#[test]
fn integral_l_apostrophe_integrale() {
    assert!(is_integral_release("One Piece - L'intégrale"));
    assert!(is_integral_release("L'INTEGRALE de Naruto"));
}

#[test]
fn integral_with_surrounding_brackets() {
    assert!(is_integral_release("[Intégrale] Bleach"));
    assert!(is_integral_release("Naruto (Complete)"));
}

#[test]
fn integral_partial_word_not_matched() {
    // "completement" contains "complet" but should not match as whole word
    assert!(!is_integral_release("completement different"));
    // "integralement" should not match
    assert!(!is_integral_release("integralement refait"));
}

#[test]
fn match_title_volumes_basic() {
    let (matched, all) = match_title_volumes("One Piece T05", &[3, 5, 7]);
    assert_eq!(matched, vec![5]);
    assert_eq!(all, vec![5]);
}

#[test]
fn match_title_volumes_no_match() {
    let (matched, all) = match_title_volumes("One Piece T05", &[3, 7, 9]);
    assert!(matched.is_empty());
    assert_eq!(all, vec![5]);
}

#[test]
fn match_title_volumes_integral_returns_all_missing() {
    let (matched, all) = match_title_volumes("One Piece Intégrale", &[1, 2, 3, 10, 20]);
    assert_eq!(matched, vec![1, 2, 3, 10, 20]);
    assert!(all.is_empty(), "integral should have empty all_volumes");
}

#[test]
fn match_title_volumes_integral_empty_missing() {
    let (matched, all) = match_title_volumes("One Piece Intégrale", &[]);
    assert!(matched.is_empty());
    assert!(all.is_empty());
}

#[test]
fn match_title_volumes_range_partial_match() {
    let (matched, _all) = match_title_volumes("Dragon Ball T01-T10", &[5, 8, 15]);
    assert_eq!(sorted(matched), vec![5, 8]);
}

#[test]
fn match_missing_volumes_maps_correctly() {
    let releases = vec![
        ProwlarrRawRelease {
            guid: "a".into(),
            title: "Naruto T05".into(),
            size: 100,
            download_url: None,
            indexer: None,
            seeders: None,
            leechers: None,
            publish_date: None,
            protocol: None,
            info_url: None,
            categories: None,
        },
        ProwlarrRawRelease {
            guid: "b".into(),
            title: "Naruto T99".into(),
            size: 200,
            download_url: None,
            indexer: None,
            seeders: None,
            leechers: None,
            publish_date: None,
            protocol: None,
            info_url: None,
            categories: None,
        },
    ];
    let missing = vec![
        MissingVolumeInput { volume_number: Some(5), title: None },
        MissingVolumeInput { volume_number: Some(10), title: None },
    ];
    let result = match_missing_volumes(releases, &missing);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].matched_missing_volumes, Some(vec![5]));
    assert!(result[1].matched_missing_volumes.is_none());
    assert_eq!(result[0].all_volumes, vec![5]);
    assert_eq!(result[1].all_volumes, vec![99]);
}

#[test]
fn match_missing_volumes_with_none_volume() {
    let missing = vec![
        MissingVolumeInput { volume_number: None, title: Some("test".into()) },
    ];
    let releases = vec![
        ProwlarrRawRelease {
            guid: "a".into(),
            title: "Naruto T05".into(),
            size: 100,
            download_url: None,
            indexer: None,
            seeders: None,
            leechers: None,
            publish_date: None,
            protocol: None,
            info_url: None,
            categories: None,
        },
    ];
    let result = match_missing_volumes(releases, &missing);
    // No missing_numbers to match against, so matched should be None
    assert!(result[0].matched_missing_volumes.is_none());
}

#[test]
fn is_integral_with_grave_accent_e() {
    assert!(is_integral_release("Série Intègrale")); // è instead of é
}

// ── Wiremock integration tests ──────────────────────────────────────────

use wiremock::matchers::{method, path, header};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_raw_release(guid: &str, title: &str, size: i64) -> serde_json::Value {
    serde_json::json!({
        "guid": guid,
        "title": title,
        "size": size,
        "downloadUrl": "https://example.com/download/123",
        "indexer": "TestIndexer",
        "seeders": 42,
        "leechers": 5,
        "publishDate": "2025-01-15T10:00:00Z",
        "protocol": "torrent",
        "infoUrl": "https://example.com/info/123",
        "categories": [{"id": 7030, "name": "Comics"}]
    })
}

#[tokio::test]
async fn wiremock_search_returns_releases_with_correct_parsing() {
    let server = MockServer::start().await;

    let body = serde_json::json!([
        make_raw_release("guid-1", "One Piece T05 [FR]", 500_000_000),
        make_raw_release("guid-2", "Naruto Tome 12 [CBZ]", 300_000_000),
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .and(header("X-Api-Key", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    let result = do_prowlarr_search(
        &server.uri(),
        "test-key",
        "One Piece",
        &[7030],
        None,
    )
    .await
    .expect("search should succeed");

    assert_eq!(result.results.len(), 2);
    assert_eq!(result.query, "One Piece");

    let r0 = &result.results[0];
    assert_eq!(r0.guid, "guid-1");
    assert_eq!(r0.title, "One Piece T05 [FR]");
    assert_eq!(r0.size, 500_000_000);
    assert_eq!(r0.download_url.as_deref(), Some("https://example.com/download/123"));
    assert_eq!(r0.indexer.as_deref(), Some("TestIndexer"));
    assert_eq!(r0.seeders, Some(42));
    assert_eq!(r0.leechers, Some(5));
    assert_eq!(r0.protocol.as_deref(), Some("torrent"));
    assert!(r0.matched_missing_volumes.is_none());
    assert_eq!(r0.all_volumes, vec![5]);

    let r1 = &result.results[1];
    assert_eq!(r1.guid, "guid-2");
    assert_eq!(r1.all_volumes, vec![12]);
}

#[tokio::test]
async fn wiremock_search_with_missing_volumes_matching() {
    let server = MockServer::start().await;

    let body = serde_json::json!([
        make_raw_release("guid-1", "One Piece T05 [FR]", 500_000_000),
        make_raw_release("guid-2", "One Piece T01-T10 [FR]", 1_000_000_000),
        make_raw_release("guid-3", "One Piece T99 [FR]", 200_000_000),
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    let missing = vec![
        MissingVolumeInput { volume_number: Some(5), title: None },
        MissingVolumeInput { volume_number: Some(8), title: None },
        MissingVolumeInput { volume_number: Some(15), title: None },
    ];

    let result = do_prowlarr_search(
        &server.uri(),
        "test-key",
        "One Piece",
        &[7030],
        Some(&missing),
    )
    .await
    .expect("search should succeed");

    assert_eq!(result.results.len(), 3);

    // T05 matches missing volume 5
    assert_eq!(result.results[0].matched_missing_volumes, Some(vec![5]));

    // T01-T10 matches missing volumes 5 and 8 (not 15, which is outside range)
    let matched = result.results[1].matched_missing_volumes.as_ref().unwrap();
    let mut sorted_matched = matched.clone();
    sorted_matched.sort();
    assert_eq!(sorted_matched, vec![5, 8]);

    // T99 matches nothing
    assert!(result.results[2].matched_missing_volumes.is_none());
}

#[tokio::test]
async fn wiremock_search_empty_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let result = do_prowlarr_search(
        &server.uri(),
        "test-key",
        "Nonexistent Series",
        &[7030, 7020],
        None,
    )
    .await
    .expect("search should succeed");

    assert!(result.results.is_empty());
    assert_eq!(result.query, "Nonexistent Series");
}

#[tokio::test]
async fn wiremock_test_connection_success() {
    let server = MockServer::start().await;

    let indexers = serde_json::json!([
        {"id": 1, "name": "Indexer A"},
        {"id": 2, "name": "Indexer B"},
        {"id": 3, "name": "Indexer C"},
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v1/indexer"))
        .and(header("X-Api-Key", "my-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&indexers))
        .mount(&server)
        .await;

    let result = do_prowlarr_test(&server.uri(), "my-api-key")
        .await
        .expect("test should succeed");

    assert!(result.success);
    assert_eq!(result.indexer_count, Some(3));
    assert!(result.message.contains("3 indexers"));
}

#[tokio::test]
async fn wiremock_test_connection_unauthorized() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/indexer"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let result = do_prowlarr_test(&server.uri(), "bad-key")
        .await
        .expect("test should return a response (not Err)");

    assert!(!result.success);
    assert!(result.indexer_count.is_none());
    assert!(result.message.contains("401"));
}

#[tokio::test]
async fn wiremock_search_http_500_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let err = do_prowlarr_search(
        &server.uri(),
        "test-key",
        "query",
        &[7030],
        None,
    )
    .await
    .unwrap_err();

    assert!(
        err.message.contains("500") || err.message.contains("Internal Server Error"),
        "error should mention 500, got: {}", err.message
    );
}

#[tokio::test]
async fn wiremock_search_invalid_json_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
        .mount(&server)
        .await;

    let err = do_prowlarr_search(
        &server.uri(),
        "test-key",
        "query",
        &[7030],
        None,
    )
    .await
    .unwrap_err();

    assert!(
        err.message.contains("parse") || err.message.contains("Parse"),
        "error should mention parse failure, got: {}", err.message
    );
}

#[tokio::test]
async fn wiremock_search_with_integral_release_matching() {
    let server = MockServer::start().await;

    let body = serde_json::json!([
        make_raw_release("guid-int", "One Piece Intégrale [CBZ]", 5_000_000_000_i64),
    ]);

    Mock::given(method("GET"))
        .and(path("/api/v1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&server)
        .await;

    let missing = vec![
        MissingVolumeInput { volume_number: Some(1), title: None },
        MissingVolumeInput { volume_number: Some(50), title: None },
        MissingVolumeInput { volume_number: Some(100), title: None },
    ];

    let result = do_prowlarr_search(
        &server.uri(),
        "test-key",
        "One Piece",
        &[7030],
        Some(&missing),
    )
    .await
    .expect("search should succeed");

    assert_eq!(result.results.len(), 1);
    let release = &result.results[0];
    // Integral release should match ALL missing volumes
    let matched = release.matched_missing_volumes.as_ref().unwrap();
    assert_eq!(matched, &vec![1, 50, 100]);
    // all_volumes should be empty for integral releases
    assert!(release.all_volumes.is_empty());
}
