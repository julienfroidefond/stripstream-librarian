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
        "totalItems": 2,
        "items": [
            {
                "id": "vol1",
                "volumeInfo": {
                    "title": "Blacksad Vol. 1",
                    "authors": ["Juan Diaz Canales", "Juanjo Guarnido"],
                    "publisher": "Dark Horse Comics",
                    "publishedDate": "2010-03-15",
                    "description": "A noir detective story starring a cat.",
                    "imageLinks": {
                        "thumbnail": "http://books.google.com/thumb1.jpg"
                    }
                }
            },
            {
                "id": "vol2",
                "volumeInfo": {
                    "title": "Blacksad Vol. 2",
                    "authors": ["Juan Diaz Canales"],
                    "publisher": "Dark Horse Comics",
                    "publishedDate": "2012-06-01"
                }
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path_regex("/books/v1/volumes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let candidates = search_series_impl("Blacksad", &config, &mock_server.uri())
        .await
        .unwrap();

    assert!(
        !candidates.is_empty(),
        "should return at least one candidate"
    );
    let best = &candidates[0];
    assert_eq!(best.title, "Blacksad");
    assert!(best.authors.contains(&"Juan Diaz Canales".to_string()));
    assert!(best.authors.contains(&"Juanjo Guarnido".to_string()));
    assert_eq!(best.publishers, vec!["Dark Horse Comics".to_string()]);
    assert_eq!(best.start_year, Some(2010));
    assert_eq!(best.total_volumes, Some(2));
    assert!(best.cover_url.is_some());
    // HTTP should be upgraded to HTTPS
    assert!(best.cover_url.as_ref().unwrap().starts_with("https://"));
}

#[tokio::test]
async fn search_series_empty_results() {
    let mock_server = MockServer::start().await;

    let body = serde_json::json!({ "totalItems": 0 });

    Mock::given(method("GET"))
        .and(path_regex("/books/v1/volumes"))
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

    // Mock the initial volume fetch
    let volume_body = serde_json::json!({
        "id": "vol_abc",
        "volumeInfo": {
            "title": "Blacksad Vol. 1"
        }
    });

    Mock::given(method("GET"))
        .and(path_regex("^/books/v1/volumes/vol_abc$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&volume_body))
        .mount(&mock_server)
        .await;

    // Mock the search for series volumes
    let search_body = serde_json::json!({
        "totalItems": 3,
        "items": [
            {
                "id": "id1",
                "volumeInfo": {
                    "title": "Blacksad Vol. 1",
                    "authors": ["Juan Diaz Canales"],
                    "industryIdentifiers": [
                        { "type": "ISBN_13", "identifier": "9781234567890" }
                    ],
                    "pageCount": 120,
                    "language": "en",
                    "publishedDate": "2010-03-15",
                    "imageLinks": { "thumbnail": "http://example.com/thumb1.jpg" }
                }
            },
            {
                "id": "id2",
                "volumeInfo": {
                    "title": "Blacksad Vol. 2",
                    "authors": ["Juan Diaz Canales"],
                    "industryIdentifiers": [
                        { "type": "ISBN_10", "identifier": "1234567890" }
                    ],
                    "pageCount": 130,
                    "language": "en",
                    "publishedDate": "2012-06-01"
                }
            },
            {
                "id": "id3",
                "volumeInfo": {
                    "title": "Blacksad Vol. 3",
                    "authors": ["Juan Diaz Canales"],
                    "language": "en"
                }
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path_regex("^/books/v1/volumes$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let books = get_series_books_impl("vol_abc", &config, &mock_server.uri())
        .await
        .unwrap();

    assert_eq!(books.len(), 3);

    // Books should be sorted by volume number
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[1].volume_number, Some(2));
    assert_eq!(books[2].volume_number, Some(3));

    // Check first book fields
    assert_eq!(books[0].title, "Blacksad Vol. 1");
    assert_eq!(books[0].authors, vec!["Juan Diaz Canales"]);
    assert_eq!(books[0].isbn, Some("9781234567890".to_string()));
    assert_eq!(books[0].page_count, Some(120));
    assert_eq!(books[0].language, Some("en".to_string()));

    // Second book has ISBN_10
    assert_eq!(books[1].isbn, Some("1234567890".to_string()));

    // Third book has no ISBN
    assert!(books[2].isbn.is_none());
}

#[tokio::test]
async fn get_series_books_empty_search_results() {
    let mock_server = MockServer::start().await;

    let volume_body = serde_json::json!({
        "id": "vol_xyz",
        "volumeInfo": {
            "title": "Some Obscure Book"
        }
    });

    Mock::given(method("GET"))
        .and(path_regex("^/books/v1/volumes/vol_xyz$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&volume_body))
        .mount(&mock_server)
        .await;

    // Search returns no items
    let search_body = serde_json::json!({ "totalItems": 0 });

    Mock::given(method("GET"))
        .and(path_regex("^/books/v1/volumes$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&search_body))
        .mount(&mock_server)
        .await;

    let config = test_config();
    let books = get_series_books_impl("vol_xyz", &config, &mock_server.uri())
        .await
        .unwrap();

    // Should fall back to the single volume
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "Some Obscure Book");
}
