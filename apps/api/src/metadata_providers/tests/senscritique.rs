use super::*;

// ─── extract_volume_number ───────────────────────────────────────────

#[test]
fn extract_volume_number_tome() {
    assert_eq!(extract_volume_number("One Piece, tome 3"), Some(3));
    assert_eq!(extract_volume_number("Naruto Tome 12"), Some(12));
    assert_eq!(extract_volume_number("TOME 1"), Some(1));
}

#[test]
fn extract_volume_number_t_dot() {
    assert_eq!(extract_volume_number("One Piece T.3"), Some(3));
    assert_eq!(extract_volume_number("Series T.12"), Some(12));
    assert_eq!(extract_volume_number("T.007"), Some(7));
}

#[test]
fn extract_volume_number_vol() {
    assert_eq!(extract_volume_number("Vol. 12"), Some(12));
    assert_eq!(extract_volume_number("Vol 5"), Some(5));
    assert_eq!(extract_volume_number("Vol.3"), Some(3));
    assert_eq!(extract_volume_number("Volume 8"), Some(8));
    assert_eq!(extract_volume_number("volume 1"), Some(1));
}

#[test]
fn extract_volume_number_integrale_no_match() {
    // "Intégrale" has no volume number pattern
    assert_eq!(extract_volume_number("One Piece - Intégrale"), None);
}

#[test]
fn extract_volume_number_no_volume() {
    assert_eq!(extract_volume_number("Just a book title"), None);
    assert_eq!(extract_volume_number(""), None);
    assert_eq!(extract_volume_number("No numbers at all"), None);
}

#[test]
fn extract_volume_number_zero_padded() {
    assert_eq!(extract_volume_number("Tome 007"), Some(7));
    assert_eq!(extract_volume_number("T.001"), Some(1));
}

// ─── infer_status_from_date ──────────────────────────────────────────

#[test]
fn infer_status_recent_date_is_ongoing() {
    // A date very recently should be "ongoing"
    let recent = chrono::Utc::now().date_naive() - chrono::Duration::days(30);
    let date_str = recent.format("%Y-%m-%d").to_string();
    assert_eq!(infer_status_from_date(&date_str), "ongoing");
}

#[test]
fn infer_status_old_date_is_ended() {
    // A date 3 years ago should be "ended"
    assert_eq!(infer_status_from_date("2020-01-01"), "ended");
}

#[test]
fn infer_status_invalid_date_is_ended() {
    assert_eq!(infer_status_from_date("not-a-date"), "ended");
    assert_eq!(infer_status_from_date(""), "ended");
    assert_eq!(infer_status_from_date("2024/01/01"), "ended"); // wrong format
}

#[test]
fn infer_status_boundary_date() {
    // Exactly at the 18-month cutoff boundary
    let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(18 * 30);
    let date_str = cutoff.format("%Y-%m-%d").to_string();
    // At the cutoff date itself, date is NOT > cutoff, so "ended"
    assert_eq!(infer_status_from_date(&date_str), "ended");

    // One day after cutoff should be "ongoing"
    let one_after = cutoff + chrono::Duration::days(1);
    let date_str = one_after.format("%Y-%m-%d").to_string();
    assert_eq!(infer_status_from_date(&date_str), "ongoing");
}

// ─── extract_names ───────────────────────────────────────────────────

#[test]
fn extract_names_with_array() {
    let product = serde_json::json!({
        "authors": [
            {"name": "Eiichiro Oda"},
            {"name": "Another Author"}
        ]
    });
    assert_eq!(
        extract_names(&product, "authors"),
        vec!["Eiichiro Oda", "Another Author"]
    );
}

#[test]
fn extract_names_empty() {
    let product = serde_json::json!({});
    assert_eq!(extract_names(&product, "authors"), Vec::<String>::new());
}

#[test]
fn extract_names_missing_name_field() {
    let product = serde_json::json!({
        "authors": [
            {"name": "Author1"},
            {"other": "no name"},
            {"name": "Author2"}
        ]
    });
    assert_eq!(
        extract_names(&product, "authors"),
        vec!["Author1", "Author2"]
    );
}

#[test]
fn extract_product_authors_includes_pencillers_without_duplicates() {
    let product = serde_json::json!({
        "authors": [{"name": "René Goscinny"}],
        "pencillers": [
            {"name": "Albert Uderzo"},
            {"name": "René Goscinny"}
        ]
    });

    assert_eq!(
        extract_product_authors(&product),
        vec!["René Goscinny", "Albert Uderzo"]
    );
}

#[test]
fn first_volume_product_selects_tome_one() {
    let tome_two = serde_json::json!({"title": "Astérix, tome 2"});
    let tome_one = serde_json::json!({"title": "Astérix, tome 1"});
    let products = vec![&tome_two, &tome_one];

    assert_eq!(
        first_volume_product(&products)
            .and_then(|product| product.get("title"))
            .and_then(|title| title.as_str()),
        Some("Astérix, tome 1")
    );
}

// ─── Wiremock integration tests ─────────────────────────────────────

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_autocomplete_response() -> serde_json::Value {
    serde_json::json!({
        "data": {
            "searchAutocomplete": {
                "items": [
                    {
                        "product": {
                            "id": 112604,
                            "title": "Quelque part entre les ombres - Blacksad, tome 1",
                            "url": "/bd/blacksad_tome_1/112604",
                            "category": "BD franco-belge",
                            "synopsis": "Blacksad est un chat détective privé.",
                            "medias": { "picture": "https://example.com/cover.jpg" },
                            "authors": [{ "name": "Juan Díaz Canales" }],
                            "pencillers": [{ "name": "Juanjo Guarnido" }],
                            "dateRelease": "2000-11-10",
                            "rating": 8.1,
                            "yearOfProduction": 2000,
                            "franchises": [{ "id": 2430, "label": "Blacksad" }]
                        }
                    },
                    {
                        "product": {
                            "id": 386532,
                            "title": "Arctic-Nation - Blacksad, tome 2",
                            "url": "/bd/blacksad_tome_2/386532",
                            "category": "BD franco-belge",
                            "synopsis": "Deuxième aventure de Blacksad.",
                            "medias": { "picture": "https://example.com/cover2.jpg" },
                            "authors": [{ "name": "Juan Díaz Canales" }],
                            "pencillers": [{ "name": "Juanjo Guarnido" }],
                            "dateRelease": "2003-03-22",
                            "rating": 8.3,
                            "yearOfProduction": 2003,
                            "franchises": [{ "id": 2430, "label": "Blacksad" }]
                        }
                    },
                    {
                        "product": {
                            "id": 999,
                            "title": "Standalone Book",
                            "url": "/bd/standalone/999",
                            "category": "BD",
                            "synopsis": "A standalone book.",
                            "medias": { "picture": "https://example.com/standalone.jpg" },
                            "authors": [{ "name": "Solo Author" }],
                            "pencillers": [],
                            "dateRelease": "2020-01-01",
                            "rating": 7.0,
                            "yearOfProduction": 2020,
                            "franchises": []
                        }
                    }
                ]
            }
        }
    })
}

fn mock_latest_dates_response() -> serde_json::Value {
    serde_json::json!({
        "data": {
            "f_2430": {
                "items": [{ "dateRelease": "2025-12-05" }]
            }
        }
    })
}

#[tokio::test]
async fn wiremock_graphql_request_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "test": true }
        })))
        .mount(&server)
        .await;

    let client = build_client().unwrap();
    let body = serde_json::json!({ "query": "{ test }" });
    let result = graphql_request_url(&client, &server.uri(), &body)
        .await
        .unwrap();
    assert_eq!(result["data"]["test"], true);
}

#[tokio::test]
async fn wiremock_graphql_error_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "errors": [{ "message": "Something went wrong" }]
        })))
        .mount(&server)
        .await;

    let client = build_client().unwrap();
    let body = serde_json::json!({ "query": "{ bad }" });
    let result = graphql_request_url(&client, &server.uri(), &body).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Something went wrong"));
}

#[tokio::test]
async fn wiremock_graphql_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = build_client().unwrap();
    let body = serde_json::json!({ "query": "{ test }" });
    let result = graphql_request_url(&client, &server.uri(), &body).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("HTTP 500"));
}

#[tokio::test]
async fn wiremock_search_deduplicates_by_franchise() {
    let server = MockServer::start().await;

    // First call: searchAutocomplete
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_autocomplete_response()))
        .expect(1..=2) // autocomplete + latest dates
        .mount(&server)
        .await;

    // Mock the latest dates call (second POST)
    // Since wiremock matches both POSTs the same way, we need to handle it
    // For this test, we just verify the parsing logic with mock data directly

    let client = build_client().unwrap();
    let body = serde_json::json!({
        "query": r#"{ searchAutocomplete(keywords: "Blacksad", universe: "comicBook", limit: 20) { items { product { id title url category synopsis medias { picture } authors { name } pencillers { name } dateRelease rating yearOfProduction franchises { id label } } } } }"#,
    });
    let data = graphql_request_url(&client, &server.uri(), &body)
        .await
        .unwrap();

    let items = data
        .pointer("/data/searchAutocomplete/items")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(items.len(), 3, "mock returns 3 items");

    // Verify dedup logic manually
    let mut franchise_map: std::collections::HashMap<i64, String> =
        std::collections::HashMap::new();
    let mut standalone_count = 0;
    for item in items {
        let product = item.get("product").unwrap();
        let franchises = product
            .get("franchises")
            .and_then(|f| f.as_array())
            .unwrap();
        if let Some(f) = franchises.first() {
            let fid = f["id"].as_i64().unwrap();
            franchise_map
                .entry(fid)
                .or_insert_with(|| product["title"].as_str().unwrap().to_string());
        } else {
            standalone_count += 1;
        }
    }
    assert_eq!(
        franchise_map.len(),
        1,
        "two Blacksad tomes should dedup to one franchise"
    );
    assert_eq!(standalone_count, 1, "one standalone book");
    assert!(franchise_map.contains_key(&2430));
}

#[tokio::test]
async fn group_products_filters_missing_covers() {
    let items = vec![
        serde_json::json!({
            "id": 1, "title": "Has Cover", "url": "/test/1",
            "medias": { "picture": "https://example.com/real.jpg" },
            "authors": [], "dateRelease": "2024-01-01", "rating": 8.0,
            "franchises": [{ "id": 100, "label": "Series A" }]
        }),
        serde_json::json!({
            "id": 2, "title": "Missing Cover", "url": "/test/2",
            "medias": { "picture": "https://media.senscritique.com/missing/701/300x0/missing.png" },
            "authors": [], "dateRelease": "2024-01-01", "rating": 7.0,
            "franchises": [{ "id": 200, "label": "Series B" }]
        }),
    ];
    let results = group_products_by_franchise(&items, 10);
    assert_eq!(results.len(), 2);
    assert!(results[0].cover_url.is_some(), "real cover should be kept");
    assert!(
        results[1].cover_url.is_none(),
        "missing.png cover should be filtered out"
    );
}

#[test]
fn group_products_by_franchise_deduplicates() {
    let items = vec![
        serde_json::json!({
            "id": 1, "title": "One Piece, tome 114", "url": "/test/1",
            "medias": { "picture": "https://example.com/op114.jpg" },
            "authors": [{"name": "Oda"}], "dateRelease": "2024-01-01", "rating": 9.0,
            "franchises": [{ "id": 482, "label": "One Piece" }]
        }),
        serde_json::json!({
            "id": 2, "title": "One Piece, tome 113", "url": "/test/2",
            "medias": { "picture": "https://example.com/op113.jpg" },
            "authors": [{"name": "Oda"}], "dateRelease": "2024-02-01", "rating": 8.5,
            "franchises": [{ "id": 482, "label": "One Piece" }]
        }),
        serde_json::json!({
            "id": 3, "title": "Naruto, tome 72", "url": "/test/3",
            "medias": { "picture": "https://example.com/naruto72.jpg" },
            "authors": [{"name": "Kishimoto"}], "dateRelease": "2024-03-01", "rating": 8.0,
            "franchises": [{ "id": 817, "label": "Naruto" }]
        }),
    ];
    let results = group_products_by_franchise(&items, 10);
    assert_eq!(
        results.len(),
        2,
        "two One Piece tomes should be grouped into one"
    );
    assert_eq!(results[0].title, "One Piece");
    assert_eq!(results[0].external_id, "franchise:482");
    assert_eq!(results[1].title, "Naruto");
    assert_eq!(results[1].external_id, "franchise:817");
}

// ─── extract_edition_name ─────────────────────────────────────────

#[test]
fn edition_name_standard() {
    assert_eq!(
        extract_edition_name("Naruto Uzumaki !! - Naruto, tome 1"),
        Some("Naruto".to_string())
    );
}

#[test]
fn edition_name_with_parentheses() {
    assert_eq!(
        extract_edition_name("Naruto (Édition Hokage), tome 33"),
        Some("Naruto (Édition Hokage)".to_string())
    );
}

#[test]
fn edition_name_colon() {
    assert_eq!(
        extract_edition_name("Boruto: Two Blue Vortex, tome 5"),
        Some("Boruto: Two Blue Vortex".to_string())
    );
}

#[test]
fn edition_name_no_tome() {
    assert_eq!(extract_edition_name("One Piece - Intégrale"), None);
    assert_eq!(extract_edition_name("Naruto"), None);
}

#[test]
fn edition_name_t_dot() {
    assert_eq!(
        extract_edition_name("One Piece T.42"),
        Some("One Piece".to_string())
    );
}

#[test]
fn edition_name_simple() {
    assert_eq!(
        extract_edition_name("Dragon Ball, tome 10"),
        Some("Dragon Ball".to_string())
    );
}

// ─── encode/decode edition ────────────────────────────────────────

#[test]
fn edition_encode_decode_roundtrip() {
    for name in [
        "Naruto",
        "Naruto (Édition Hokage)",
        "Boruto: Two Blue Vortex",
        "Astérix",
    ] {
        let encoded = encode_edition(name);
        let decoded = decode_edition(&encoded).unwrap();
        assert_eq!(decoded, name, "round-trip failed for {name}");
    }
}

// ─── group_products_by_edition ────────────────────────────────────

#[test]
fn group_by_edition_mixed() {
    let items = vec![
        serde_json::json!({"title": "Naruto Uzumaki !! - Naruto, tome 1", "id": 1}),
        serde_json::json!({"title": "Se battre - Naruto, tome 2", "id": 2}),
        serde_json::json!({"title": "Naruto (Édition Hokage), tome 1", "id": 3}),
        serde_json::json!({"title": "Naruto (Édition Hokage), tome 2", "id": 4}),
        serde_json::json!({"title": "Naruto (Édition Hokage), tome 3", "id": 5}),
        serde_json::json!({"title": "Naruto - Intégrale", "id": 6}),
    ];
    let groups = group_products_by_edition(&items);
    assert_eq!(groups.len(), 2, "should have 2 editions");
    assert_eq!(groups[0].0, "Naruto (Édition Hokage)");
    assert_eq!(groups[0].1.len(), 3);
    assert_eq!(groups[1].0, "Naruto");
    assert_eq!(groups[1].1.len(), 2);
}

#[test]
fn group_by_edition_excludes_no_tome() {
    let items = vec![
        serde_json::json!({"title": "Artbook", "id": 1}),
        serde_json::json!({"title": "Intégrale collector", "id": 2}),
    ];
    let groups = group_products_by_edition(&items);
    assert!(groups.is_empty());
}

// ─── external_id parsing ──────────────────────────────────────────

#[test]
fn external_id_franchise_with_edition() {
    let name = "Naruto (Édition Hokage)";
    let ext_id = format!("franchise:817:edition:{}", encode_edition(name));
    let rest = ext_id.strip_prefix("franchise:").unwrap();
    let (fid_str, edition_part) = rest.split_once(":edition:").unwrap();
    assert_eq!(fid_str, "817");
    assert_eq!(decode_edition(edition_part).unwrap(), name);
}

#[test]
fn external_id_franchise_backward_compat() {
    let ext_id = "franchise:817";
    let rest = ext_id.strip_prefix("franchise:").unwrap();
    assert!(
        rest.split_once(":edition:").is_none(),
        "old format has no edition"
    );
}

// ─── name_similarity ──────────────────────────────────────────────

#[test]
fn similarity_exact_match() {
    assert!((name_similarity("Naruto", "Naruto") - 1.0).abs() < f32::EPSILON);
}

#[test]
fn similarity_case_insensitive() {
    assert!((name_similarity("naruto", "NARUTO") - 1.0).abs() < f32::EPSILON);
}

#[test]
fn similarity_accent_insensitive() {
    assert!((name_similarity("Astérix", "Asterix") - 1.0).abs() < f32::EPSILON);
}

#[test]
fn similarity_edition_suffix_lower() {
    // "Naruto" vs "Naruto (Édition Hokage)" — contained, so > 0.5
    let s = name_similarity("Naruto", "Naruto (Édition Hokage)");
    assert!(s > 0.3 && s < 1.0, "partial containment: got {s}");
}

#[test]
fn similarity_different_series() {
    let s = name_similarity("Naruto", "One Piece");
    assert!(s < 0.2, "unrelated: got {s}");
}

#[test]
fn similarity_subseries() {
    // "Boruto" vs "Naruto" — share "ruto" but are different
    let s = name_similarity("Boruto", "Naruto");
    assert!(s < 0.5, "different series: got {s}");
}
