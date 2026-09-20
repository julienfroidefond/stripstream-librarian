use super::*;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> ProviderConfig {
    ProviderConfig {
        api_key: Some("test_key".to_string()),
        language: "en".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn search_series_parses_candidates() {
    let mock_server = MockServer::start().await;

    let body = serde_json::json!({
        "results": [
            {
                "id": 12345,
                "name": "Blacksad",
                "description": "<p>A noir detective story.</p>",
                "publisher": { "name": "Dark Horse Comics" },
                "start_year": "2010",
                "count_of_issues": 6,
                "image": {
                    "medium_url": "https://comicvine.example.com/thumb1.jpg"
                },
                "site_detail_url": "https://comicvine.example.com/blacksad/"
            },
            {
                "id": 67890,
                "name": "Blacksad: The Collected Stories",
                "publisher": { "name": "Europe Comics" },
                "start_year": "2014",
                "count_of_issues": 2,
                "image": {
                    "small_url": "https://comicvine.example.com/thumb2.jpg"
                },
                "site_detail_url": "https://comicvine.example.com/blacksad-collected/"
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path_regex("/api/search/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let candidates = search_series_impl("Blacksad", &config, &mock_server.uri())
        .await
        .unwrap();

    assert_eq!(candidates.len(), 2);

    let best = &candidates[0];
    assert_eq!(best.external_id, "12345");
    assert_eq!(best.title, "Blacksad");
    assert!(best.authors.is_empty());
    assert_eq!(best.publishers, vec!["Dark Horse Comics".to_string()]);
    assert_eq!(best.start_year, Some(2010));
    assert_eq!(best.total_volumes, Some(6));
    assert_eq!(
        best.cover_url,
        Some("https://comicvine.example.com/thumb1.jpg".to_string())
    );
    assert_eq!(
        best.external_url,
        Some("https://comicvine.example.com/blacksad/".to_string())
    );
    // Description should have HTML stripped
    assert_eq!(
        best.description,
        Some("A noir detective story.".to_string())
    );

    let second = &candidates[1];
    assert_eq!(second.external_id, "67890");
    assert_eq!(second.publishers, vec!["Europe Comics".to_string()]);
    // Falls back to small_url when medium_url is absent
    assert_eq!(
        second.cover_url,
        Some("https://comicvine.example.com/thumb2.jpg".to_string())
    );
}

#[tokio::test]
async fn search_series_empty_results() {
    let mock_server = MockServer::start().await;

    let body = serde_json::json!({ "results": [] });

    Mock::given(method("GET"))
        .and(path_regex("/api/search/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let candidates = search_series_impl("nonexistent_xyz_123", &config, &mock_server.uri())
        .await
        .unwrap();

    assert!(
        candidates.is_empty(),
        "should return empty vec for no results"
    );
}

#[tokio::test]
async fn search_series_no_results_key() {
    let mock_server = MockServer::start().await;

    // Response without "results" key at all
    let body = serde_json::json!({ "status_code": 1, "error": "OK" });

    Mock::given(method("GET"))
        .and(path_regex("/api/search/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let candidates = search_series_impl("anything", &config, &mock_server.uri())
        .await
        .unwrap();

    assert!(candidates.is_empty());
}

#[tokio::test]
async fn search_series_requires_api_key() {
    let config = ProviderConfig {
        api_key: None,
        language: "en".to_string(),
        ..Default::default()
    };

    let result = search_series_impl("Blacksad", &config, "http://unused").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("API key"));
}

#[tokio::test]
async fn search_series_rejects_empty_api_key() {
    let config = ProviderConfig {
        api_key: Some("".to_string()),
        language: "en".to_string(),
        ..Default::default()
    };

    let result = search_series_impl("Blacksad", &config, "http://unused").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("API key"));
}

#[tokio::test]
async fn get_series_books_parses_issues() {
    let mock_server = MockServer::start().await;

    let body = serde_json::json!({
        "results": [
            {
                "id": 100,
                "name": "Somewhere Within the Shadows",
                "issue_number": "1",
                "description": "<p>First issue.</p>",
                "image": {
                    "medium_url": "https://comicvine.example.com/issue1.jpg"
                },
                "cover_date": "2010-03-15",
                "site_detail_url": "https://comicvine.example.com/issue/100/",
                "person_credits": [
                    { "name": "Juan Diaz Canales" },
                    { "name": "Juanjo Guarnido" }
                ]
            },
            {
                "id": 101,
                "name": "Arctic Nation",
                "issue_number": "2",
                "description": null,
                "image": {
                    "small_url": "https://comicvine.example.com/issue2_small.jpg"
                },
                "cover_date": "2012-06-01"
            },
            {
                "id": 102,
                "name": "Red Soul",
                "issue_number": "3",
                "image": {},
                "cover_date": null
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path_regex("/api/issues/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let books = get_series_books_impl("12345", &config, &mock_server.uri())
        .await
        .unwrap();

    assert_eq!(books.len(), 3);

    // First issue
    assert_eq!(books[0].external_book_id, "100");
    assert_eq!(books[0].title, "Somewhere Within the Shadows");
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[0].summary, Some("First issue.".to_string()));
    assert_eq!(
        books[0].cover_url,
        Some("https://comicvine.example.com/issue1.jpg".to_string())
    );
    assert_eq!(books[0].publish_date, Some("2010-03-15".to_string()));
    assert_eq!(
        books[0].authors,
        vec![
            "Juan Diaz Canales".to_string(),
            "Juanjo Guarnido".to_string()
        ]
    );
    // LOCKED: ComicVine issues expose neither ISBN nor page count.
    // See docs/KNOWN_ISSUES.md §1.
    assert!(books[0].isbn.is_none());
    assert!(books[0].page_count.is_none());

    // Second issue - falls back to small_url
    assert_eq!(books[1].external_book_id, "101");
    assert_eq!(books[1].title, "Arctic Nation");
    assert_eq!(books[1].volume_number, Some(2));
    assert!(books[1].summary.is_none());
    assert_eq!(
        books[1].cover_url,
        Some("https://comicvine.example.com/issue2_small.jpg".to_string())
    );

    // Third issue - no cover, no date
    assert_eq!(books[2].external_book_id, "102");
    assert_eq!(books[2].volume_number, Some(3));
    assert!(books[2].cover_url.is_none());
    assert!(books[2].publish_date.is_none());
}

#[tokio::test]
async fn get_series_books_empty_results() {
    let mock_server = MockServer::start().await;

    let body = serde_json::json!({ "results": [] });

    Mock::given(method("GET"))
        .and(path_regex("/api/issues/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let books = get_series_books_impl("99999", &config, &mock_server.uri())
        .await
        .unwrap();

    assert!(books.is_empty());
}

#[tokio::test]
async fn get_series_books_requires_api_key() {
    let config = ProviderConfig {
        api_key: None,
        language: "en".to_string(),
        ..Default::default()
    };

    let result = get_series_books_impl("12345", &config, "http://unused").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("API key"));
}
