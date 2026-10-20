use super::*;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn parses_series_and_volume_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/ajax/search/series/.*"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                r#"[{"id":"22444/les-geants","nom":"Les Géants","nomvo":"","couv":"T_53064.JPG","note":"3"},{"id":"25070/simon-et-lucie","nom":"Simon & Lucie","nomvo":"","couv":null,"note":null}]"#,
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path_regex(r"/series/22444/les-geants"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<html><meta name="Description" content="Une aventure."><meta property="og:image" content="https://www.bdtheque.com/repupload/T/T_53064.JPG"><h1>Les Géants</h1><script type="application/ld+json">{"@context":"https://schema.org/","@type":"Book","name":"Les Géants","aggregateRating":{"@type":"AggregateRating","ratingValue":"3","bestRating":"5","worstRating":"1","ratingCount":"2"}}</script><table><tr><td>Scénario</td><td><a href="/recherche/series/auteurs=Lylian">Lylian</a></td></tr><tr><td>Dessin</td><td><a href="/recherche/series/auteurs=Paul%20Drouin">Drouin (Paul)</a></td></tr><tr><td>Editeur / Collection</td><td><a href="/recherche/series/editeur=Glenat">Glénat</a> <a href="/recherche/series/collection=tcho">Tchô ! la collec...</a></td></tr><tr><td>Date de parution</td><td>26 Août <a href="/recherche/series/annee=2020">2020</a></td></tr><tr><td>Statut histoire</td><td><a href="/recherche/series/histoire=cycles-termines">Série en cours - cycle(s) terminé(s)</a> <span>12 tomes parus</span></td></tr></table></html>"#)).mount(&server).await;
    let results = search_series_impl("Les Géants", &server.uri())
        .await
        .unwrap();
    assert_eq!(results[0].external_id, "22444/les-geants");
    assert_eq!(results[0].title, "Les Géants");
    assert_eq!(
        results[0].cover_url.as_deref(),
        Some("https://www.bdtheque.com/repupload/T/T_53064.JPG")
    );
    assert_eq!(results[0].authors, vec!["Lylian", "Paul Drouin"]);
    assert_eq!(results[0].publishers, vec!["Glénat"]);
    assert_eq!(results[0].start_year, Some(2020));
    assert_eq!(results[0].total_volumes, Some(12));
    assert_eq!(results[0].confidence, 1.0);
    assert_eq!(results[1].external_id, "25070/simon-et-lucie");
    assert_eq!(results[1].cover_url, None);
    assert_eq!(
        results[0]
            .metadata_json
            .get("publishers")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(1)
    );

    let series = get_series_impl("22444/les-geants", &server.uri())
        .await
        .unwrap();
    assert_eq!(series.title, "Les Géants");
    assert_eq!(series.total_volumes, Some(12));
    assert_eq!(series.start_year, Some(2020));
    assert_eq!(series.authors, vec!["Lylian", "Paul Drouin"]);
    assert_eq!(series.publishers, vec!["Glénat"]);
    assert_eq!(
        series.metadata_json.get("status").and_then(|v| v.as_str()),
        Some("Série en cours - cycle(s) terminé(s)")
    );
    assert_eq!(
        series.metadata_json.get("rating").and_then(|v| v.as_f64()),
        Some(3.0)
    );
    assert_eq!(
        series
            .metadata_json
            .get("rating_count")
            .and_then(|v| v.as_i64()),
        Some(2)
    );
    assert_eq!(
        series
            .metadata_json
            .get("rating_scale")
            .and_then(|v| v.as_f64()),
        Some(5.0)
    );

    Mock::given(method("GET")).and(path_regex(r"/ajax/series/tomes/22444/0"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<div class="card"><h5>1 - Erin</h5><img class="cover" src="/erin.jpg"><small>Date de parution : 26 Août 2020 | Pagination : 48 | ISBN : 9782344039403</small><p class="card-text">Scénario : Lylian</p></div>"#)).mount(&server).await;
    Mock::given(method("GET")).and(path_regex(r"/ajax/series/tomes/22444/1"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<div class="card"><h5>2 - Tome 2</h5><img class="cover" src="/t2.jpg"><small>ISBN : 9782344039404</small></div>"#)).mount(&server).await;
    Mock::given(method("GET"))
        .and(path_regex(r"/ajax/series/tomes/22444/2"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&server)
        .await;
    let books = get_series_books_impl("22444/les-geants", &server.uri())
        .await
        .unwrap();
    assert_eq!(books.len(), 2);
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[0].isbn.as_deref(), Some("9782344039403"));
    assert_eq!(books[1].volume_number, Some(2));
    assert_eq!(books[1].isbn.as_deref(), Some("9782344039404"));
}

#[test]
fn normalizes_author_names() {
    assert_eq!(normalize_author_name("Goscinny (René)"), "René Goscinny");
    assert_eq!(normalize_author_name("Conrad (Didier)"), "Didier Conrad");
    assert_eq!(
        normalize_author_name("Le Gall (Nicolas)"),
        "Nicolas Le Gall"
    );
    assert_eq!(normalize_author_name("Lylian"), "Lylian");
    assert_eq!(normalize_author_name("  Mobidic  "), "Mobidic");
}

#[tokio::test]
async fn propagates_http_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let error = search_series_impl("Les Géants", &server.uri())
        .await
        .unwrap_err();
    assert!(error.contains("403"));
}

#[tokio::test]
async fn search_falls_back_to_typeahead_cover() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/ajax/search/series/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[{"id":"207/asterix","nom":"Astérix","nomvo":"","couv":"T_610.JPG","note":"4.6"}]"#,
        ))
        .mount(&server)
        .await;
    // The enrichment call to /series/{id} is left unmocked (404) on purpose.
    let results = search_series_impl("Astérix", &server.uri()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].external_id, "207/asterix");
    let expected_cover = format!("{}/repupload/T/T_610.JPG", server.uri());
    assert_eq!(
        results[0].cover_url.as_deref(),
        Some(expected_cover.as_str())
    );
    let expected_url = format!("{}/series/207/asterix", server.uri());
    assert_eq!(
        results[0].external_url.as_deref(),
        Some(expected_url.as_str())
    );
}

#[tokio::test]
async fn null_search_payload_returns_no_results() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/ajax/search/series/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string("null"))
        .mount(&server)
        .await;
    let results = search_series_impl("Introuvable", &server.uri())
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn invalid_search_payload_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/ajax/search/series/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>oops</html>"))
        .mount(&server)
        .await;
    let error = search_series_impl("Astérix", &server.uri())
        .await
        .unwrap_err();
    assert!(error.contains("invalid BDTheque search response"));
}

#[tokio::test]
async fn blank_query_skips_the_http_call() {
    // Port 9 is unreachable: any request would fail, so an empty result proves
    // the query was short-circuited.
    let results = search_series_impl("   ", "http://127.0.0.1:9")
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[test]
fn search_url_encodes_query_as_a_path_segment() {
    assert_eq!(
        search_url("https://www.bdtheque.com", "Astérix").unwrap(),
        "https://www.bdtheque.com/ajax/search/series/Ast%C3%A9rix"
    );
    assert_eq!(
        search_url("https://www.bdtheque.com/", "  Lanfeust / Trolls ").unwrap(),
        "https://www.bdtheque.com/ajax/search/series/Lanfeust%20Trolls"
    );
}

#[test]
fn parse_search_hits_skips_entries_without_id_or_title() {
    let hits = parse_search_hits(
        r#"[{"id":"","nom":"Sans id","couv":null},{"id":"1/x","nom":"","couv":null},{"id":"1/x","nom":"Valide","couv":"T_1.JPG"}]"#,
    )
    .unwrap();
    let candidates: Vec<_> = hits
        .into_iter()
        .filter_map(|hit| candidate_from_hit(hit, "https://example.com", "valide"))
        .collect();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "Valide");
}

#[test]
fn parses_json_ld_aggregate_rating() {
    let html = r#"<script type="application/ld+json">{"@type":"Book","name":"X","aggregateRating":{"ratingValue":"3","bestRating":"5","worstRating":"1","ratingCount":"2"}}</script>"#;
    let doc = scraper::Html::parse_document(html);
    assert_eq!(parse_aggregate_rating(&doc), Some((3.0, 2, 5.0)));
}

#[test]
fn parses_json_ld_numeric_aggregate_rating() {
    let html = r#"<script type="application/ld+json">{"aggregateRating":{"ratingValue":4.2,"bestRating":5,"ratingCount":10}}</script>"#;
    let doc = scraper::Html::parse_document(html);
    assert_eq!(parse_aggregate_rating(&doc), Some((4.2, 10, 5.0)));
}

#[test]
fn missing_aggregate_rating_returns_none() {
    let html = r#"<script type="application/ld+json">{"@type":"Book","name":"X"}</script>"#;
    let doc = scraper::Html::parse_document(html);
    assert_eq!(parse_aggregate_rating(&doc), None);
}

#[test]
fn zero_vote_aggregate_rating_is_ignored() {
    let html = r#"<script type="application/ld+json">{"aggregateRating":{"ratingValue":"0","bestRating":"5","ratingCount":"0"}}</script>"#;
    let doc = scraper::Html::parse_document(html);
    assert_eq!(parse_aggregate_rating(&doc), None);
}
