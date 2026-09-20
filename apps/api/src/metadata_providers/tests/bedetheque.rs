use super::*;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config() -> ProviderConfig {
    ProviderConfig {
        api_key: None,
        language: "fr".to_string(),
        ..Default::default()
    }
}

// ── search_series_impl ──────────────────────────────────────────────

#[tokio::test]
async fn search_series_parses_candidates_from_html() {
    let server = MockServer::start().await;

    let html = r#"
    <html>
    <head><title>Recherche</title></head>
    <body>
        <div>
            <a href="/serie-3-BD-Blacksad.html">Blacksad</a>
            <a href="/serie-42-BD-Asterix.html">Astérix</a>
            <a href="/not-a-serie.html">Ignored</a>
        </div>
    </body>
    </html>"#;

    Mock::given(method("GET"))
        .and(path_regex(r"/search/tout.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    // Mock the series detail pages (enrichment for top 3)
    let detail_html = r#"
    <html>
    <head><meta name="description" content="Tout sur la série Blacksad : Une série policière animalière."></head>
    <body>
        <span itemprop="author">Díaz Canales, Juan</span>
        <span itemprop="illustrator">Guarnido, Juanjo</span>
        <span itemprop="publisher">Dargaud</span>
        <span class="style-serie">Policier, Animalier</span>
        <span class="parution-serie">Série en cours</span>
        Tomes : 7
        <meta itemprop="datePublished" content="2000-11-01">
        <img itemprop="image" src="/cache/thb_couv/Couv_123.jpg">
    </body>
    </html>"#;

    Mock::given(method("GET"))
        .and(path_regex(r"/serie-\d+-.*__10000\.html"))
        .respond_with(ResponseTemplate::new(200).set_body_string(detail_html))
        .mount(&server)
        .await;

    let results = search_series_impl("Blacksad", &config(), &server.uri())
        .await
        .unwrap();

    assert_eq!(results.len(), 2);

    // First result should be Blacksad (higher confidence for exact match)
    assert_eq!(results[0].title, "Blacksad");
    assert_eq!(results[0].external_id, "3");
    assert!(results[0].confidence > results[1].confidence);

    // Second result
    assert_eq!(results[1].title, "Astérix");
    assert_eq!(results[1].external_id, "42");

    // Enrichment should have populated details for top candidates
    assert!(results[0].description.is_some());
    assert!(results[0]
        .description
        .as_ref()
        .unwrap()
        .contains("policière animalière"));
    assert!(!results[0].authors.is_empty());
    assert!(!results[0].publishers.is_empty());
}

#[tokio::test]
async fn search_series_detects_rate_limiting() {
    let server = MockServer::start().await;

    let html = "<html><head><title></title></head><body></body></html>";

    Mock::given(method("GET"))
        .and(path_regex(r"/search/tout.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    let result = search_series_impl("test", &config(), &server.uri()).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("rate-limited"));
}

#[tokio::test]
async fn search_series_cover_url_uses_base_url() {
    let server = MockServer::start().await;

    let html = r#"
    <html>
    <head><title>Results</title></head>
    <body>
        <a href="/serie-99-BD-TestSerie.html">TestSerie</a>
    </body>
    </html>"#;

    Mock::given(method("GET"))
        .and(path_regex(r"/search/tout.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    // Return 404 for enrichment so it's skipped gracefully
    Mock::given(method("GET"))
        .and(path_regex(r"/serie-99-.*__10000\.html"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let results = search_series_impl("TestSerie", &config(), &server.uri())
        .await
        .unwrap();
    assert_eq!(results.len(), 1);

    // No cover is fabricated when the enrichment request fails (404).
    assert!(results[0].cover_url.is_none());
}

// ── get_series_books_impl ───────────────────────────────────────────

#[tokio::test]
async fn get_series_books_parses_albums() {
    let server = MockServer::start().await;

    let html = r#"
    <html>
    <head><title>Blacksad</title></head>
    <body>
        <img itemprop="image" src="/cache/thb_couv/Couv_100.jpg">
        <img itemprop="image" src="/cache/thb_couv/Couv_101.jpg">
        <div class="album-main">
            <a class="titre" href="/BD-Blacksad-Tome-1-Quelque-part-entre-les-ombres-1063.html"
               title="Quelque part entre les ombres">Quelque part entre les ombres</a>
            <span itemprop="author">Díaz Canales, Juan</span>
            <span itemprop="isbn">978-2-87129-410-1</span>
            <span itemprop="numberOfPages">48</span>
            <meta itemprop="datePublished" content="2000-11-01">
        </div>
        <div class="album-main">
            <a class="titre" href="/BD-Blacksad-Tome-2-Arctic-Nation-1064.html"
               title="Arctic-Nation">Arctic-Nation</a>
            <span itemprop="author">Díaz Canales, Juan</span>
            <span itemprop="isbn">978-2-87129-456-9</span>
            <span itemprop="numberOfPages">56</span>
            <meta itemprop="datePublished" content="2003-03-01">
        </div>
    </body>
    </html>"#;

    Mock::given(method("GET"))
        .and(path_regex(r"/serie-\d+-BD-Serie__10000\.html"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    let books = get_series_books_impl("3", &config(), &server.uri())
        .await
        .unwrap();

    assert_eq!(books.len(), 2);

    assert_eq!(books[0].title, "Quelque part entre les ombres");
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[0].external_book_id, "1063");
    assert_eq!(books[0].isbn.as_deref(), Some("978-2-87129-410-1"));
    assert_eq!(books[0].page_count, Some(48));
    assert_eq!(books[0].publish_date.as_deref(), Some("2000-11-01"));

    // Cover URL from pre-collected covers
    let cover = books[0].cover_url.as_ref().unwrap();
    assert!(
        cover.contains("Couv_100"),
        "first book should get first cover, got: {cover}"
    );

    assert_eq!(books[1].title, "Arctic-Nation");
    assert_eq!(books[1].volume_number, Some(2));
}

#[tokio::test]
async fn get_series_books_rate_limited() {
    let server = MockServer::start().await;

    let html = "<html><head><title></title></head><body></body></html>";

    Mock::given(method("GET"))
        .and(path_regex(r"/serie-.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    let result = get_series_books_impl("3", &config(), &server.uri()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("rate-limited"));
}

#[tokio::test]
async fn get_series_books_skips_non_tome_albums() {
    let server = MockServer::start().await;

    // Album URL without "Tome-N-" pattern should be skipped
    let html = r#"
    <html>
    <head><title>Test</title></head>
    <body>
        <div class="album-main">
            <a class="titre" href="/BD-Blacksad-INT-Integrale-9999.html"
               title="Intégrale">Intégrale</a>
        </div>
        <div class="album-main">
            <a class="titre" href="/BD-Blacksad-Tome-1-Title-1000.html"
               title="Tome 1">Tome 1</a>
        </div>
    </body>
    </html>"#;

    Mock::given(method("GET"))
        .and(path_regex(r"/serie-.*"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&server)
        .await;

    let books = get_series_books_impl("1", &config(), &server.uri())
        .await
        .unwrap();
    // Only the Tome-1 album should be kept, the INT one should be filtered out
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].title, "Tome 1");
}

// ── Pure function tests ─────────────────────────────────────────────

#[test]
fn parse_serie_href_extracts_id_and_slug() {
    let (id, slug) = parse_serie_href("/serie-3-BD-Blacksad.html").unwrap();
    assert_eq!(id, "3");
    assert_eq!(slug, "Blacksad");
}

#[test]
fn parse_serie_href_with_pagination_suffix() {
    let (id, slug) = parse_serie_href("/serie-3-BD-Blacksad__10000.html").unwrap();
    assert_eq!(id, "3");
    assert_eq!(slug, "Blacksad");
}

#[test]
fn parse_serie_href_returns_none_for_invalid() {
    assert!(parse_serie_href("/not-a-serie.html").is_none());
    assert!(parse_serie_href("").is_none());
}

#[test]
fn normalize_for_url_removes_diacritics() {
    assert_eq!(normalize_for_url("Astérix"), "Asterix");
    assert_eq!(normalize_for_url("François"), "Francois");
    assert_eq!(normalize_for_url("naïve café"), "naive cafe");
}

#[test]
fn compute_confidence_exact_match() {
    assert_eq!(compute_confidence("Blacksad", "blacksad"), 1.0);
}

#[test]
fn compute_confidence_prefix_match() {
    assert_eq!(compute_confidence("Blacksad - Tome 1", "blacksad"), 0.85);
}

#[test]
fn compute_confidence_contains_match() {
    assert_eq!(
        compute_confidence("Les Aventures de Blacksad", "blacksad"),
        0.7
    );
}

#[test]
fn compute_confidence_normalized_match() {
    // "Légendaires (Les)" normalized == "légendaires" == "Les Légendaires" normalized
    assert_eq!(
        compute_confidence("Légendaires (Les)", "Les Légendaires"),
        1.0
    );
}

#[test]
fn is_real_author_filters_placeholders() {
    assert!(!is_real_author("<Anonyme>"));
    assert!(!is_real_author("Collectif"));
    assert!(is_real_author("Jean Dupont"));
}

#[test]
fn urlencoded_encodes_spaces_and_specials() {
    assert_eq!(urlencoded("hello world"), "hello+world");
    assert_eq!(urlencoded("café"), "caf%C3%A9");
}
