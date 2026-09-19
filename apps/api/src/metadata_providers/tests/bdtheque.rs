use super::*;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn parses_series_and_volume_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"/recherche/series/.*"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                r#"<figure><a href="/series/22444/les-geants" title="Accéder à la série BD Les Géants"><span class="helper"></span><img alt="Couverture de la série Les Géants" data-echo="https://www.bdtheque.com/repupload/T/T_53064.JPG"></a></figure>"#,
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path_regex(r"/series/22444/les-geants"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<html><meta name="Description" content="Une aventure."><meta property="og:image" content="https://www.bdtheque.com/repupload/T/T_53064.JPG"><h1>Les Géants</h1><table><tr><td>Scénario</td><td><a href="/recherche/series/auteurs=Lylian">Lylian</a></td></tr><tr><td>Dessin</td><td><a href="/recherche/series/auteurs=Paul%20Drouin">Drouin (Paul)</a></td></tr><tr><td>Editeur / Collection</td><td><a href="/recherche/series/editeur=Glenat">Glénat</a> <a href="/recherche/series/collection=tcho">Tchô ! la collec...</a></td></tr><tr><td>Date de parution</td><td>26 Août <a href="/recherche/series/annee=2020">2020</a></td></tr><tr><td>Statut histoire</td><td><a href="/recherche/series/histoire=cycles-termines">Série en cours - cycle(s) terminé(s)</a> <span>12 tomes parus</span></td></tr></table></html>"#)).mount(&server).await;
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

    Mock::given(method("GET")).and(path_regex(r"/ajax/series/tomes/22444/0"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"<div class="card"><h5>1 - Erin</h5><img class="cover" src="/erin.jpg"><small>Date de parution : 26 Août 2020 | Pagination : 48 | ISBN : 9782344039403</small><p class="card-text">Scénario : Lylian</p></div>"#)).mount(&server).await;
    let books = get_series_books_impl("22444/les-geants", &server.uri())
        .await
        .unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].volume_number, Some(1));
    assert_eq!(books[0].isbn.as_deref(), Some("9782344039403"));
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
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let error = search_series_impl("Les Géants", &server.uri())
        .await
        .unwrap_err();
    assert!(error.contains("403"));
}
