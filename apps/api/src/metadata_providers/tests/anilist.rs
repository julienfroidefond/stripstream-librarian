use super::*;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap()
}

fn mock_search_response() -> serde_json::Value {
    serde_json::json!({
        "data": {
            "Page": {
                "media": [
                    {
                        "id": 20,
                        "title": { "romaji": "Naruto", "english": "Naruto", "native": "NARUTO" },
                        "description": "A ninja story",
                        "coverImage": { "large": "https://example.com/naruto.jpg", "medium": "https://example.com/naruto_sm.jpg" },
                        "startDate": { "year": 1999 },
                        "status": "FINISHED",
                        "volumes": 72,
                        "chapters": 700,
                        "staff": {
                            "edges": [
                                {
                                    "node": { "name": { "full": "Masashi Kishimoto" } },
                                    "role": "Story & Art"
                                }
                            ]
                        },
                        "siteUrl": "https://anilist.co/manga/20/Naruto",
                        "genres": ["Action", "Adventure"]
                    },
                    {
                        "id": 21,
                        "title": { "romaji": "Naruto: Chibi Sasuke", "english": null, "native": null },
                        "description": null,
                        "coverImage": { "medium": "https://example.com/chibi.jpg" },
                        "startDate": { "year": 2014 },
                        "status": "FINISHED",
                        "volumes": 3,
                        "chapters": null,
                        "staff": { "edges": [] },
                        "siteUrl": "https://anilist.co/manga/21/Chibi",
                        "genres": ["Comedy"]
                    }
                ]
            }
        }
    })
}

fn mock_detail_response_finished() -> serde_json::Value {
    serde_json::json!({
        "data": {
            "Media": {
                "id": 20,
                "title": { "romaji": "Naruto", "english": "Naruto", "native": "NARUTO" },
                "description": "A ninja story",
                "coverImage": { "large": "https://example.com/naruto.jpg" },
                "startDate": { "year": 1999 },
                "status": "FINISHED",
                "volumes": 72,
                "chapters": 700,
                "staff": {
                    "edges": [
                        {
                            "node": { "name": { "full": "Masashi Kishimoto" } },
                            "role": "Story & Art"
                        }
                    ]
                },
                "siteUrl": "https://anilist.co/manga/20/Naruto",
                "genres": ["Action", "Adventure"]
            }
        }
    })
}

fn mock_detail_response_ongoing() -> serde_json::Value {
    serde_json::json!({
        "data": {
            "Media": {
                "id": 30013,
                "title": { "romaji": "One Piece", "english": "One Piece", "native": "ONE PIECE" },
                "description": "A pirate story",
                "coverImage": { "large": "https://example.com/onepiece.jpg" },
                "startDate": { "year": 1997 },
                "status": "RELEASING",
                "volumes": null,
                "chapters": null,
                "staff": {
                    "edges": [
                        {
                            "node": { "name": { "full": "Eiichiro Oda" } },
                            "role": "Story & Art"
                        }
                    ]
                },
                "siteUrl": "https://anilist.co/manga/30013/One-Piece",
                "genres": ["Action", "Adventure", "Comedy"]
            }
        }
    })
}

fn mock_trending_response() -> serde_json::Value {
    serde_json::json!({
        "data": {
            "Page": {
                "media": [
                    {
                        "id": 105778,
                        "title": { "romaji": "Oshi no Ko", "english": "Oshi No Ko", "native": null },
                        "description": "Idol manga",
                        "coverImage": { "large": "https://example.com/oshinoko.jpg" },
                        "startDate": { "year": 2020 },
                        "status": "FINISHED",
                        "volumes": 16,
                        "chapters": 166,
                        "staff": {
                            "edges": [
                                { "node": { "name": { "full": "Aka Akasaka" } }, "role": "Original Story" },
                                { "node": { "name": { "full": "Mengo Yokoyari" } }, "role": "Art" }
                            ]
                        },
                        "siteUrl": "https://anilist.co/manga/105778",
                        "genres": ["Drama", "Mystery", "Supernatural"]
                    },
                    {
                        "id": 30002,
                        "title": { "romaji": "Berserk", "english": "Berserk", "native": null },
                        "description": "Dark fantasy",
                        "coverImage": { "large": "https://example.com/berserk.jpg" },
                        "startDate": { "year": 1989 },
                        "status": "RELEASING",
                        "volumes": null,
                        "chapters": 376,
                        "staff": {
                            "edges": [
                                { "node": { "name": { "full": "Kentaro Miura" } }, "role": "Story & Art" }
                            ]
                        },
                        "siteUrl": "https://anilist.co/manga/30002",
                        "genres": ["Action", "Drama", "Fantasy"]
                    }
                ]
            }
        }
    })
}

// ─── graphql_request_url tests ──────────────────────────────────────────

#[tokio::test]
async fn wiremock_graphql_request_url_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": { "test": true } })),
        )
        .mount(&server)
        .await;

    let client = build_client();
    let result = graphql_request_url(
        &client,
        &server.uri(),
        "{ test }",
        serde_json::json!({}),
    )
    .await
    .unwrap();
    assert_eq!(result["data"]["test"], true);
}

#[tokio::test]
async fn wiremock_graphql_request_url_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let client = build_client();
    let result = graphql_request_url(
        &client,
        &server.uri(),
        "{ test }",
        serde_json::json!({}),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("500"), "error should mention status code: {err}");
}

#[tokio::test]
async fn wiremock_graphql_request_url_invalid_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = build_client();
    let result = graphql_request_url(
        &client,
        &server.uri(),
        "{ test }",
        serde_json::json!({}),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to parse"));
}

// ─── search_series tests ────────────────────────────────────────────────

#[tokio::test]
async fn wiremock_search_series_parses_candidates() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_search_response()),
        )
        .mount(&server)
        .await;

    let results = search_series_impl_url("naruto", &server.uri()).await.unwrap();

    assert_eq!(results.len(), 2);

    // First result should be "Naruto" (exact match = highest confidence)
    let naruto = &results[0];
    assert_eq!(naruto.external_id, "20");
    assert_eq!(naruto.title, "Naruto");
    assert_eq!(naruto.authors, vec!["Masashi Kishimoto"]);
    assert_eq!(naruto.total_volumes, Some(72));
    assert_eq!(naruto.start_year, Some(1999));
    assert_eq!(naruto.cover_url.as_deref(), Some("https://example.com/naruto.jpg"));
    assert_eq!(naruto.external_url.as_deref(), Some("https://anilist.co/manga/20/Naruto"));
    assert_eq!(naruto.metadata_json["status"], "FINISHED");
    assert_eq!(naruto.metadata_json["volumes"], 72);
    assert_eq!(naruto.metadata_json["chapters"], 700);
    assert_eq!(naruto.metadata_json["volume_source"], "volumes");

    // Second result uses romaji (english is null), falls back correctly
    let chibi = &results[1];
    assert_eq!(chibi.external_id, "21");
    assert_eq!(chibi.title, "Naruto: Chibi Sasuke");
    assert!(chibi.authors.is_empty());
    assert_eq!(chibi.total_volumes, Some(3));
    assert_eq!(chibi.metadata_json["volume_source"], "volumes");
}

#[tokio::test]
async fn wiremock_search_series_empty_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "Page": { "media": [] } }
            })),
        )
        .mount(&server)
        .await;

    let results = search_series_impl_url("nonexistent_manga_xyz", &server.uri())
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn wiremock_search_series_null_media() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "Page": { "media": null } }
            })),
        )
        .mount(&server)
        .await;

    let results = search_series_impl_url("test", &server.uri()).await.unwrap();
    assert!(results.is_empty());
}

// ─── fetch_trending tests ───────────────────────────────────────────────

#[tokio::test]
async fn wiremock_fetch_trending_parses_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_trending_response()),
        )
        .mount(&server)
        .await;

    let results = fetch_trending_url(10, &server.uri()).await.unwrap();

    assert_eq!(results.len(), 2);

    let oshi = &results[0];
    assert_eq!(oshi.external_id, "105778");
    assert_eq!(oshi.title, "Oshi No Ko");
    assert_eq!(oshi.authors, vec!["Aka Akasaka", "Mengo Yokoyari"]);
    assert_eq!(oshi.total_volumes, Some(16));
    assert_eq!(oshi.metadata_json["status"], "FINISHED");
    assert_eq!(oshi.metadata_json["volume_source"], "volumes");

    let berserk = &results[1];
    assert_eq!(berserk.external_id, "30002");
    assert_eq!(berserk.title, "Berserk");
    // volumes is null, should fall back to chapters
    assert_eq!(berserk.total_volumes, Some(376));
    assert_eq!(berserk.metadata_json["volume_source"], "chapters");
    assert_eq!(berserk.metadata_json["status"], "RELEASING");
}

#[tokio::test]
async fn wiremock_fetch_trending_empty() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "Page": { "media": [] } }
            })),
        )
        .mount(&server)
        .await;

    let results = fetch_trending_url(10, &server.uri()).await.unwrap();
    assert!(results.is_empty());
}

// ─── get_series_books tests ─────────────────────────────────────────────

#[tokio::test]
async fn wiremock_get_series_books_finished() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_detail_response_finished()),
        )
        .mount(&server)
        .await;

    let books = get_series_books_impl_url("20", &server.uri()).await.unwrap();

    assert_eq!(books.len(), 72);
    assert_eq!(books[0].title, "Naruto Vol. 1");
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[0].authors, vec!["Masashi Kishimoto"]);
    assert!(books[0].summary.is_some());
    assert!(books[0].cover_url.is_some());
    assert_eq!(books[0].language.as_deref(), Some("ja"));

    assert_eq!(books[71].title, "Naruto Vol. 72");
    assert_eq!(books[71].volume_number, Some(72));
    // Only vol 1 gets description and cover
    assert!(books[71].summary.is_none());
    assert!(books[71].cover_url.is_none());
}

#[tokio::test]
async fn wiremock_get_series_books_ongoing_no_volumes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_detail_response_ongoing()),
        )
        .mount(&server)
        .await;

    let books = get_series_books_impl_url("30013", &server.uri()).await.unwrap();

    // Both volumes and chapters are null, so no book entries generated
    assert!(books.is_empty());
}

#[tokio::test]
async fn wiremock_get_series_books_invalid_id() {
    let result = get_series_books_impl_url("not_a_number", "http://unused").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid AniList ID"));
}

#[tokio::test]
async fn wiremock_get_series_books_chapters_fallback() {
    let server = MockServer::start().await;
    // Media with no volumes but has chapters
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "Media": {
                        "id": 999,
                        "title": { "romaji": "Web Comic", "english": null },
                        "description": null,
                        "coverImage": { "medium": "https://example.com/wc.jpg" },
                        "startDate": { "year": 2020 },
                        "status": "RELEASING",
                        "volumes": null,
                        "chapters": 5,
                        "staff": { "edges": [] },
                        "siteUrl": null,
                        "genres": []
                    }
                }
            })),
        )
        .mount(&server)
        .await;

    let books = get_series_books_impl_url("999", &server.uri()).await.unwrap();
    assert_eq!(books.len(), 5, "should fall back to chapters count");
    assert_eq!(books[0].title, "Web Comic Vol. 1");
    assert_eq!(books[4].title, "Web Comic Vol. 5");
}

// ─── Pure function tests ────────────────────────────────────────────────

#[test]
fn test_compute_confidence_exact_match() {
    assert_eq!(compute_confidence("Naruto", "naruto"), 1.0);
}

#[test]
fn test_compute_confidence_prefix() {
    assert_eq!(compute_confidence("Naruto Shippuden", "naruto"), 0.8);
}

#[test]
fn test_compute_confidence_contains() {
    assert_eq!(compute_confidence("The Art of Naruto", "naruto"), 0.7);
}

#[test]
fn test_extract_authors_filters_by_role() {
    let media = serde_json::json!({
        "staff": {
            "edges": [
                { "node": { "name": { "full": "Author A" } }, "role": "Story" },
                { "node": { "name": { "full": "Artist B" } }, "role": "Art" },
                { "node": { "name": { "full": "Editor C" } }, "role": "Editor" },
                { "node": { "name": { "full": "Creator D" } }, "role": "Original Creator" }
            ]
        }
    });
    let authors = extract_authors(&media);
    assert_eq!(authors, vec!["Author A", "Artist B", "Creator D"]);
}

#[test]
fn test_extract_authors_deduplicates() {
    let media = serde_json::json!({
        "staff": {
            "edges": [
                { "node": { "name": { "full": "Same Person" } }, "role": "Story" },
                { "node": { "name": { "full": "Same Person" } }, "role": "Art" }
            ]
        }
    });
    let authors = extract_authors(&media);
    assert_eq!(authors, vec!["Same Person"]);
}

#[test]
fn test_extract_genres() {
    let media = serde_json::json!({
        "genres": ["Action", "Comedy", "Drama"]
    });
    assert_eq!(extract_genres(&media), vec!["Action", "Comedy", "Drama"]);
}

#[test]
fn test_extract_genres_missing() {
    let media = serde_json::json!({});
    assert!(extract_genres(&media).is_empty());
}

#[test]
fn test_parse_media_to_candidate_volumes_source() {
    // With volumes
    let media = serde_json::json!({
        "id": 1, "title": { "english": "Test" },
        "description": null, "coverImage": {}, "startDate": {},
        "status": "FINISHED", "volumes": 10, "chapters": 100,
        "staff": { "edges": [] }, "siteUrl": null, "genres": []
    });
    let c = parse_media_to_candidate(&media, 0.5).unwrap();
    assert_eq!(c.total_volumes, Some(10));
    assert_eq!(c.metadata_json["volume_source"], "volumes");

    // Without volumes, falls back to chapters
    let media2 = serde_json::json!({
        "id": 2, "title": { "romaji": "Test2" },
        "description": null, "coverImage": {}, "startDate": {},
        "status": "RELEASING", "volumes": null, "chapters": 50,
        "staff": { "edges": [] }, "siteUrl": null, "genres": []
    });
    let c2 = parse_media_to_candidate(&media2, 0.5).unwrap();
    assert_eq!(c2.total_volumes, Some(50));
    assert_eq!(c2.metadata_json["volume_source"], "chapters");

    // Neither volumes nor chapters
    let media3 = serde_json::json!({
        "id": 3, "title": { "romaji": "Test3" },
        "description": null, "coverImage": {}, "startDate": {},
        "status": "RELEASING", "volumes": null, "chapters": null,
        "staff": { "edges": [] }, "siteUrl": null, "genres": []
    });
    let c3 = parse_media_to_candidate(&media3, 0.5).unwrap();
    assert_eq!(c3.total_volumes, None);
    assert_eq!(c3.metadata_json["volume_source"], "unknown");
}
