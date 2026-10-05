use super::*;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn parses_series_and_album_details() {
    let server = MockServer::start().await;
    let search_body = format!(
        r#"{{"bests":[{{"url":"{uri}/series/bd/36511-les-geants","text":"Les Géants (fr)"}}],"series":[]}}"#,
        uri = server.uri()
    );
    Mock::given(method("GET"))
        .and(path_regex(r"/search/ajax"))
        .respond_with(ResponseTemplate::new(200).set_body_string(search_body))
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path_regex(r"/series/bd/36511-les-geants"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<style>.background-series { background: url("https://static.bdphile.fr/images/media/cover/0138/138208.jpg") rgba(246, 246, 246, 0.8); }</style><h1>Les Géants</h1><div class="synopsis">Une aventure.</div><div class="albumBlocCompact"><div class="album-stats right"><div class="right"><div class="bar-wrapper"><div class="bar"></div></div></div><div style="font-size: 72px;">3.7<span style="font-size: 20px;">/ 5</span></div><div class="stats-details text-right clear"><span class="flaticon-rate tooltip" title="11 votes">11</span></div></div></div><div class="albumBlocCompact"><div class="album-stats right"><div style="font-size: 72px;">4.0<span style="font-size: 20px;">/ 5</span></div><div class="stats-details text-right clear"><span class="flaticon-rate tooltip" title="9 votes">9</span></div></div></div><dl><dt><span class="tooltip" title="Auteurs"></span></dt><dd><div><a href="https://www.bdphile.fr/author/view/123-lylian">Lylian</a> (Scénario)</div><div><a href="https://www.bdphile.fr/author/view/456-paul-drouin">Paul Drouin</a> (Dessin)</div><div><a href="https://www.bdphile.fr/author/view/789-coloriste">Coloriste</a> (Couleurs)</div><div><a href="https://www.bdphile.fr/author/view/790-studio">Studio X</a> (Encrage)</div></dd><dt><span class="tooltip" title="Publication"></span></dt><dd>De 2020 à aujourd'hui <br/> En cours - 12 tomes parus (6 prévus)</dd><dt><span class="tooltip" title="Éditeurs"></span></dt><dd><a href="https://www.bdphile.fr/publisher/view/25-glenat">Glénat</a></dd></dl><div id="detail_view"><table><tbody><tr><td>1</td><td><a href="/album/bd/138208-les-geants-1-erin">Erin</a></td><td>Glénat</td><td>26 août 2020</td></tr></tbody></table></div>"#)).mount(&server).await;
    let results = search_series_impl("Les Géants", &server.uri())
        .await
        .unwrap();
    assert_eq!(results[0].external_id, "bd/36511-les-geants");
    assert_eq!(results[0].title, "Les Géants");
    assert_eq!(
        results[0].cover_url.as_deref(),
        Some("https://static.bdphile.fr/images/media/cover/0138/138208.jpg")
    );
    assert_eq!(results[0].authors, vec!["Lylian", "Paul Drouin"]);
    assert_eq!(results[0].publishers, vec!["Glénat"]);
    assert_eq!(results[0].start_year, Some(2020));
    assert_eq!(results[0].total_volumes, Some(12));

    let series = get_series_impl("bd/36511-les-geants", &server.uri())
        .await
        .unwrap();
    assert_eq!(series.title, "Les Géants");
    assert_eq!(series.total_volumes, Some(12));
    assert_eq!(series.start_year, Some(2020));
    assert_eq!(series.authors, vec!["Lylian", "Paul Drouin"]);
    assert_eq!(series.publishers, vec!["Glénat"]);
    assert_eq!(
        series.metadata_json.get("status").and_then(|v| v.as_str()),
        Some("En cours")
    );
    let series_rating = series
        .metadata_json
        .get("rating")
        .and_then(|v| v.as_f64())
        .expect("series rating");
    assert!(
        (series_rating - 3.835).abs() < 1e-9,
        "expected weighted average 3.835, got {series_rating}"
    );
    assert_eq!(
        series
            .metadata_json
            .get("rating_count")
            .and_then(|v| v.as_i64()),
        Some(20)
    );
    assert_eq!(
        series
            .metadata_json
            .get("rating_scale")
            .and_then(|v| v.as_f64()),
        Some(5.0)
    );
    assert_eq!(
        series.cover_url.as_deref(),
        Some("https://static.bdphile.fr/images/media/cover/0138/138208.jpg")
    );

    Mock::given(method("GET")).and(path_regex(r"/album/bd/138208-les-geants-1-erin"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<meta property="og:image" content="/cover.jpg"><dl><dt>Scénario</dt><dd>Lylian</dd><dt>EAN</dt><dd>9782344039403</dd><dt>Format</dt><dd>Cartonné - 48 pages</dd><dt>Date de publication</dt><dd>26 août 2020</dd></dl>"#)).mount(&server).await;
    let books = get_series_books_impl("bd/36511-les-geants", &server.uri())
        .await
        .unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[0].external_book_id, "bd/138208-les-geants-1-erin");
    assert_eq!(books[0].isbn.as_deref(), Some("9782344039403"));
    assert_eq!(books[0].page_count, Some(48));
}

#[tokio::test]
async fn propagates_http_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let error = get_series_impl("bd/404-missing", &server.uri())
        .await
        .unwrap_err();
    assert!(error.contains("404"));
}

#[tokio::test]
async fn album_fetch_error_is_propagated() {
    let server = MockServer::start().await;
    let search_body = format!(
        r#"{{"bests":[{{"url":"{uri}/series/bd/36511-les-geants","text":"Les Géants (fr)"}}],"series":[]}}"#,
        uri = server.uri()
    );
    Mock::given(method("GET"))
        .and(path_regex(r"/search/ajax"))
        .respond_with(ResponseTemplate::new(200).set_body_string(search_body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/series/bd/36511-les-geants"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<h1>Les Géants</h1><div id="detail_view"><table><tbody><tr><td>1</td><td><a href="/album/bd/138208-les-geants-1-erin">Erin</a></td><td>Glénat</td><td>26 août 2020</td></tr></tbody></table></div>"#,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"/album/bd/138208-les-geants-1-erin"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = get_series_books_impl("bd/36511-les-geants", &server.uri()).await;

    assert!(
        result.is_err(),
        "a failing album fetch must surface the error, not produce an empty book"
    );
}

#[test]
fn parses_weighted_album_ratings() {
    let html = r#"<div class="albumBlocCompact"><div class="album-stats"><div style="font-size:72px">3.7<span>/ 5</span></div><div class="stats-details"><span class="flaticon-rate" title="11 votes">11</span></div></div></div><div class="albumBlocCompact"><div class="album-stats"><div style="font-size:72px">4.0<span>/ 5</span></div><div class="stats-details"><span class="flaticon-rate" title="9 votes">9</span></div></div></div>"#;
    let doc = scraper::Html::parse_document(html);
    let (rating, count) = parse_album_ratings(&doc).expect("aggregated rating");
    assert!((rating - 3.835).abs() < 1e-9, "got {rating}");
    assert_eq!(count, 20);
}

#[test]
fn ignores_album_blocks_without_votes() {
    let html = r#"<div class="albumBlocCompact"><div class="album-stats"><div style="font-size:72px">3.7<span>/ 5</span></div><div class="stats-details"><span class="flaticon-rate" title="0 vote">0</span></div></div></div>"#;
    let doc = scraper::Html::parse_document(html);
    assert!(parse_album_ratings(&doc).is_none());
}

#[test]
fn parses_rating_over_five() {
    assert_eq!(parse_rating_over_five("3.7 / 5"), Some(3.7));
    assert_eq!(parse_rating_over_five("4/5"), Some(4.0));
    assert_eq!(parse_rating_over_five("11 0 0"), None);
    assert_eq!(parse_rating_over_five(""), None);
}
