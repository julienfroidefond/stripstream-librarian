use super::*;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> ProviderConfig {
    ProviderConfig {
        api_key: None,
        language: "en".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn search_series_parses_candidates() {
    let mock_server = MockServer::start().await;

    let body = serde_json::json!({
        "numFound": 2,
        "docs": [
            {
                "key": "/works/OL123W",
                "title": "Sandman Vol. 1",
                "author_name": ["Neil Gaiman"],
                "publisher": ["DC Comics", "Vertigo"],
                "first_publish_year": 1989,
                "cover_i": 12345
            },
            {
                "key": "/works/OL456W",
                "title": "Sandman Vol. 2",
                "author_name": ["Neil Gaiman"],
                "publisher": ["DC Comics"],
                "first_publish_year": 1990,
                "cover_i": 67890
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path_regex("/search\\.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let candidates = search_series_impl("Sandman", &config, &mock_server.uri())
        .await
        .unwrap();

    assert!(
        !candidates.is_empty(),
        "should return at least one candidate"
    );
    let best = &candidates[0];
    assert_eq!(best.title, "Sandman");
    assert!(best.authors.contains(&"Neil Gaiman".to_string()));
    assert!(best.publishers.contains(&"DC Comics".to_string()));
    assert_eq!(best.start_year, Some(1989));
    assert_eq!(best.total_volumes, None);
    assert_eq!(best.metadata_json["local_volume_count"], 2);
    assert!(best.cover_url.is_some());
    assert!(best
        .cover_url
        .as_ref()
        .unwrap()
        .contains("covers.openlibrary.org"));
    assert!(best.external_url.is_some());
    assert!(best
        .external_url
        .as_ref()
        .unwrap()
        .contains("openlibrary.org"));
}

#[tokio::test]
async fn search_series_empty_results() {
    let mock_server = MockServer::start().await;

    let body = serde_json::json!({ "numFound": 0, "docs": [] });

    Mock::given(method("GET"))
        .and(path_regex("/search\\.json"))
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
async fn get_series_books_parses_books() {
    let mock_server = MockServer::start().await;

    // Mock the work fetch
    let work_body = serde_json::json!({
        "title": "Sandman Vol. 1",
        "key": "/works/OL123W"
    });

    Mock::given(method("GET"))
        .and(path_regex("^/works/OL123W\\.json$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&work_body))
        .mount(&mock_server)
        .await;

    // Mock the search for series editions
    let search_body = serde_json::json!({
        "numFound": 3,
        "docs": [
            {
                "key": "/works/OL100W",
                "title": "Sandman Vol. 1",
                "author_name": ["Neil Gaiman"],
                "isbn": ["9780123456789"],
                "number_of_pages_median": 240,
                "cover_i": 111,
                "language": ["eng"],
                "first_publish_year": 1989
            },
            {
                "key": "/works/OL200W",
                "title": "Sandman Vol. 2",
                "author_name": ["Neil Gaiman"],
                "isbn": ["9780987654321"],
                "number_of_pages_median": 220,
                "language": ["eng"],
                "first_publish_year": 1990
            },
            {
                "key": "/works/OL300W",
                "title": "Sandman Vol. 3",
                "author_name": ["Neil Gaiman"],
                "language": ["eng"],
                "first_publish_year": 1991
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path_regex("^/search\\.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let books = get_series_books_impl("/works/OL123W", &config, &mock_server.uri())
        .await
        .unwrap();

    assert_eq!(books.len(), 3);

    // Books should be sorted by volume number
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[1].volume_number, Some(2));
    assert_eq!(books[2].volume_number, Some(3));

    // Check first book fields
    assert_eq!(books[0].title, "Sandman Vol. 1");
    assert_eq!(books[0].authors, vec!["Neil Gaiman"]);
    assert_eq!(books[0].isbn, Some("9780123456789".to_string()));
    assert_eq!(books[0].page_count, Some(240));
    assert_eq!(books[0].language, Some("eng".to_string()));
    assert_eq!(books[0].publish_date, Some("1989".to_string()));
    assert!(books[0].cover_url.is_some());

    // Third book has no ISBN or page count
    assert!(books[2].isbn.is_none());
    assert!(books[2].page_count.is_none());
}

#[tokio::test]
async fn get_series_books_empty_results() {
    let mock_server = MockServer::start().await;

    // Mock the work fetch
    let work_body = serde_json::json!({
        "title": "Some Obscure Work",
        "key": "/works/OL999W"
    });

    Mock::given(method("GET"))
        .and(path_regex("^/works/OL999W\\.json$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&work_body))
        .mount(&mock_server)
        .await;

    // Search returns empty
    let search_body = serde_json::json!({ "numFound": 0, "docs": [] });

    Mock::given(method("GET"))
        .and(path_regex("^/search\\.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let books = get_series_books_impl("/works/OL999W", &config, &mock_server.uri())
        .await
        .unwrap();

    assert!(
        books.is_empty(),
        "should return empty vec when no editions found"
    );
}
