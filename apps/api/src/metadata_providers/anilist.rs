use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

pub struct AniListProvider;

impl MetadataProvider for AniListProvider {
    fn name(&self) -> &str {
        "anilist"
    }

    fn search_series(
        &self,
        query: &str,
        config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<SeriesCandidate>, String>> + Send + '_>,
    > {
        let query = query.to_string();
        let config = config.clone();
        Box::pin(async move { search_series_impl(&query, &config).await })
    }

    fn get_series_books(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<BookCandidate>, String>> + Send + '_>,
    > {
        let external_id = external_id.to_string();
        let config = config.clone();
        Box::pin(async move { get_series_books_impl(&external_id, &config).await })
    }
}

const SEARCH_QUERY: &str = r#"
query ($search: String) {
  Page(perPage: 20) {
    media(search: $search, type: MANGA, sort: SEARCH_MATCH) {
      id
      title { romaji english native }
      description(asHtml: false)
      coverImage { large medium }
      startDate { year }
      status
      volumes
      chapters
      staff { edges { node { name { full } } role } }
      siteUrl
      genres
    }
  }
}
"#;

const DETAIL_QUERY: &str = r#"
query ($id: Int) {
  Media(id: $id, type: MANGA) {
    id
    title { romaji english native }
    description(asHtml: false)
    coverImage { large medium }
    startDate { year }
    status
    volumes
    chapters
    staff { edges { node { name { full } } role } }
    siteUrl
    genres
  }
}
"#;

const ANILIST_GRAPHQL_URL: &str = "https://graphql.anilist.co";

async fn graphql_request_url(
    client: &reqwest::Client,
    url: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "query": query,
            "variables": variables,
        }))
        .send()
        .await
        .map_err(|e| format!("AniList request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("AniList returned {status}: {text}"));
    }

    resp.json()
        .await
        .map_err(|e| format!("Failed to parse AniList response: {e}"))
}

async fn search_series_impl(
    query: &str,
    _config: &ProviderConfig,
) -> Result<Vec<SeriesCandidate>, String> {
    search_series_impl_url(query, ANILIST_GRAPHQL_URL).await
}

async fn search_series_impl_url(
    query: &str,
    url: &str,
) -> Result<Vec<SeriesCandidate>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let data = graphql_request_url(
        &client,
        url,
        SEARCH_QUERY,
        serde_json::json!({ "search": query }),
    )
    .await?;

    let media = match data
        .get("data")
        .and_then(|d| d.get("Page"))
        .and_then(|p| p.get("media"))
        .and_then(|m| m.as_array())
    {
        Some(media) => media,
        None => return Ok(vec![]),
    };

    let query_lower = query.to_lowercase();

    let mut candidates: Vec<SeriesCandidate> = media
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(|id| id.as_i64())?;
            let title_obj = m.get("title")?;
            let title = title_obj
                .get("english")
                .and_then(|t| t.as_str())
                .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))?
                .to_string();

            let description = m
                .get("description")
                .and_then(|d| d.as_str())
                .map(|d| d.replace("\\n", "\n").trim().to_string())
                .filter(|d| !d.is_empty());

            let cover_url = m
                .get("coverImage")
                .and_then(|ci| ci.get("large").or_else(|| ci.get("medium")))
                .and_then(|u| u.as_str())
                .map(String::from);

            let start_year = m
                .get("startDate")
                .and_then(|sd| sd.get("year"))
                .and_then(|y| y.as_i64())
                .map(|y| y as i32);

            let volumes = m
                .get("volumes")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);

            let chapters = m
                .get("chapters")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);

            let status = m
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("UNKNOWN")
                .to_string();

            let site_url = m
                .get("siteUrl")
                .and_then(|u| u.as_str())
                .map(String::from);

            let authors = extract_authors(m);

            let genres = extract_genres(m);

            let confidence = compute_confidence(&title, &query_lower);

            // Use volumes if known, otherwise fall back to chapters count
            let (total_volumes, volume_source) = match volumes {
                Some(v) => (Some(v), "volumes"),
                None => match chapters {
                    Some(c) => (Some(c), "chapters"),
                    None => (None, "unknown"),
                },
            };

            Some(SeriesCandidate {
                external_id: id.to_string(),
                title,
                authors,
                description,
                publishers: vec![],
                start_year,
                total_volumes,
                cover_url,
                external_url: site_url,
                confidence,
                metadata_json: serde_json::json!({
                    "status": status,
                    "chapters": chapters,
                    "volumes": volumes,
                    "volume_source": volume_source,
                    "genres": genres,
                }),
            })
        })
        .collect();

    candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(10);
    Ok(candidates)
}

async fn get_series_books_impl(
    external_id: &str,
    _config: &ProviderConfig,
) -> Result<Vec<BookCandidate>, String> {
    get_series_books_impl_url(external_id, ANILIST_GRAPHQL_URL).await
}

async fn get_series_books_impl_url(
    external_id: &str,
    url: &str,
) -> Result<Vec<BookCandidate>, String> {
    let id: i64 = external_id
        .parse()
        .map_err(|_| "invalid AniList ID".to_string())?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let data = graphql_request_url(
        &client,
        url,
        DETAIL_QUERY,
        serde_json::json!({ "id": id }),
    )
    .await?;

    let media = match data.get("data").and_then(|d| d.get("Media")) {
        Some(m) => m,
        None => return Ok(vec![]),
    };

    let title_obj = media.get("title").cloned().unwrap_or(serde_json::json!({}));
    let title = title_obj
        .get("english")
        .and_then(|t| t.as_str())
        .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))
        .unwrap_or("")
        .to_string();

    let volumes = media
        .get("volumes")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let chapters = media
        .get("chapters")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    // Use volumes if known, otherwise fall back to chapters count
    let total = volumes.or(chapters);

    let cover_url = media
        .get("coverImage")
        .and_then(|ci| ci.get("large").or_else(|| ci.get("medium")))
        .and_then(|u| u.as_str())
        .map(String::from);

    let description = media
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| d.replace("\\n", "\n").trim().to_string());

    let authors = extract_authors(media);

    // AniList doesn't have per-volume data — generate entries from volumes count (or chapters as fallback)
    let mut books = Vec::new();
    if let Some(total) = total {
        for vol in 1..=total {
            books.push(BookCandidate {
                external_book_id: format!("{}-vol-{}", external_id, vol),
                title: format!("{} Vol. {}", title, vol),
                volume_number: Some(vol),
                authors: authors.clone(),
                isbn: None,
                summary: if vol == 1 { description.clone() } else { None },
                cover_url: if vol == 1 { cover_url.clone() } else { None },
                page_count: None,
                language: Some("ja".to_string()),
                publish_date: None,
                metadata_json: serde_json::json!({}),
            });
        }
    }

    Ok(books)
}

fn extract_authors(media: &serde_json::Value) -> Vec<String> {
    let mut authors = Vec::new();
    if let Some(edges) = media
        .get("staff")
        .and_then(|s| s.get("edges"))
        .and_then(|e| e.as_array())
    {
        for edge in edges {
            let role = edge
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("");
            let role_lower = role.to_lowercase();
            if role_lower.contains("story") || role_lower.contains("art") || role_lower.contains("original") {
                if let Some(name) = edge
                    .get("node")
                    .and_then(|n| n.get("name"))
                    .and_then(|n| n.get("full"))
                    .and_then(|f| f.as_str())
                {
                    if !authors.contains(&name.to_string()) {
                        authors.push(name.to_string());
                    }
                }
            }
        }
    }
    authors
}

// ─── Trending ───────────────────────────────────────────────────────────────

const TRENDING_QUERY: &str = r#"
query ($perPage: Int) {
  Page(perPage: $perPage) {
    media(type: MANGA, sort: TRENDING_DESC) {
      id
      title { romaji english native }
      description(asHtml: false)
      coverImage { large medium }
      startDate { year }
      status
      volumes
      chapters
      staff { edges { node { name { full } } role } }
      siteUrl
      genres
    }
  }
}
"#;

/// Fetch trending manga from AniList. Does not go through the MetadataProvider trait.
pub async fn fetch_trending(limit: i32) -> Result<Vec<SeriesCandidate>, String> {
    fetch_trending_url(limit, ANILIST_GRAPHQL_URL).await
}

async fn fetch_trending_url(limit: i32, url: &str) -> Result<Vec<SeriesCandidate>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let data = graphql_request_url(
        &client,
        url,
        TRENDING_QUERY,
        serde_json::json!({ "perPage": limit.min(50) }),
    )
    .await?;

    let media = match data
        .get("data")
        .and_then(|d| d.get("Page"))
        .and_then(|p| p.get("media"))
        .and_then(|m| m.as_array())
    {
        Some(media) => media,
        None => return Ok(vec![]),
    };

    let candidates: Vec<SeriesCandidate> = media
        .iter()
        .filter_map(|m| parse_media_to_candidate(m, 0.5))
        .collect();

    Ok(candidates)
}

/// Parse a single AniList media JSON object into a SeriesCandidate.
fn parse_media_to_candidate(m: &serde_json::Value, default_confidence: f32) -> Option<SeriesCandidate> {
    let id = m.get("id").and_then(|id| id.as_i64())?;
    let title_obj = m.get("title")?;
    let title = title_obj
        .get("english")
        .and_then(|t| t.as_str())
        .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))?
        .to_string();

    let description = m
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| d.replace("\\n", "\n").trim().to_string())
        .filter(|d| !d.is_empty());

    let cover_url = m
        .get("coverImage")
        .and_then(|ci| ci.get("large").or_else(|| ci.get("medium")))
        .and_then(|u| u.as_str())
        .map(String::from);

    let start_year = m
        .get("startDate")
        .and_then(|sd| sd.get("year"))
        .and_then(|y| y.as_i64())
        .map(|y| y as i32);

    let volumes = m.get("volumes").and_then(|v| v.as_i64()).map(|v| v as i32);
    let chapters = m.get("chapters").and_then(|v| v.as_i64()).map(|v| v as i32);

    let status = m
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();

    let site_url = m.get("siteUrl").and_then(|u| u.as_str()).map(String::from);
    let authors = extract_authors(m);
    let genres = extract_genres(m);

    let (total_volumes, volume_source) = match volumes {
        Some(v) => (Some(v), "volumes"),
        None => match chapters {
            Some(c) => (Some(c), "chapters"),
            None => (None, "unknown"),
        },
    };

    Some(SeriesCandidate {
        external_id: id.to_string(),
        title,
        authors,
        description,
        publishers: vec![],
        start_year,
        total_volumes,
        cover_url,
        external_url: site_url,
        confidence: default_confidence,
        metadata_json: serde_json::json!({
            "status": status,
            "chapters": chapters,
            "volumes": volumes,
            "volume_source": volume_source,
            "genres": genres,
        }),
    })
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn extract_genres(media: &serde_json::Value) -> Vec<String> {
    media
        .get("genres")
        .and_then(|g| g.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn compute_confidence(title: &str, query: &str) -> f32 {
    let title_lower = title.to_lowercase();
    if title_lower == query {
        1.0
    } else if title_lower.starts_with(query) || query.starts_with(&title_lower) {
        0.8
    } else if title_lower.contains(query) || query.contains(&title_lower) {
        0.7
    } else {
        let common: usize = query.chars().filter(|c| title_lower.contains(*c)).count();
        let max_len = query.len().max(title_lower.len()).max(1);
        (common as f32 / max_len as f32).clamp(0.1, 0.6)
    }
}

#[cfg(test)]
mod tests {
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
}
