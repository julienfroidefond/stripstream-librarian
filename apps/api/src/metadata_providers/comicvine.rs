use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const DEFAULT_BASE_URL: &str = "https://comicvine.gamespot.com";

pub struct ComicVineProvider;

impl MetadataProvider for ComicVineProvider {
    fn name(&self) -> &str {
        "comicvine"
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
        Box::pin(async move { search_series_impl(&query, &config, DEFAULT_BASE_URL).await })
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
        Box::pin(async move { get_series_books_impl(&external_id, &config, DEFAULT_BASE_URL).await })
    }
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("StripstreamLibrarian/1.0")
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

async fn search_series_impl(
    query: &str,
    config: &ProviderConfig,
    base_url: &str,
) -> Result<Vec<SeriesCandidate>, String> {
    let api_key = config
        .api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| "ComicVine requires an API key. Configure it in Settings > Integrations.".to_string())?;

    let client = build_client()?;

    let url = format!(
        "{}/api/search/?api_key={}&format=json&resources=volume&query={}&limit=20",
        base_url,
        api_key,
        urlencoded(query)
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ComicVine request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("ComicVine returned {status}: {text}"));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse ComicVine response: {e}"))?;

    let results = match data.get("results").and_then(|r| r.as_array()) {
        Some(results) => results,
        None => return Ok(vec![]),
    };

    let query_lower = query.to_lowercase();

    let mut candidates: Vec<SeriesCandidate> = results
        .iter()
        .filter_map(|vol| {
            let name = vol.get("name").and_then(|n| n.as_str())?.to_string();
            let id = vol.get("id").and_then(|id| id.as_i64())?;
            let description = vol
                .get("description")
                .and_then(|d| d.as_str())
                .map(strip_html);
            let publisher = vol
                .get("publisher")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from);
            let start_year = vol
                .get("start_year")
                .and_then(|y| y.as_str())
                .and_then(|y| y.parse::<i32>().ok());
            let count_of_issues = vol
                .get("count_of_issues")
                .and_then(|c| c.as_i64())
                .map(|c| c as i32);
            let cover_url = vol
                .get("image")
                .and_then(|img| img.get("medium_url").or_else(|| img.get("small_url")))
                .and_then(|u| u.as_str())
                .map(String::from);
            let site_url = vol
                .get("site_detail_url")
                .and_then(|u| u.as_str())
                .map(String::from);

            let confidence = compute_confidence(&name, &query_lower);

            Some(SeriesCandidate {
                external_id: id.to_string(),
                title: name,
                authors: vec![],
                description,
                publishers: publisher.into_iter().collect(),
                start_year,
                total_volumes: count_of_issues,
                cover_url,
                external_url: site_url,
                confidence,
                metadata_json: serde_json::json!({}),
            })
        })
        .collect();

    candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(10);
    Ok(candidates)
}

async fn get_series_books_impl(
    external_id: &str,
    config: &ProviderConfig,
    base_url: &str,
) -> Result<Vec<BookCandidate>, String> {
    let api_key = config
        .api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| "ComicVine requires an API key".to_string())?;

    let client = build_client()?;

    let url = format!(
        "{}/api/issues/?api_key={}&format=json&filter=volume:{}&sort=issue_number:asc&limit=100&field_list=id,name,issue_number,description,image,cover_date,site_detail_url",
        base_url,
        api_key,
        external_id
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ComicVine request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("ComicVine returned {status}: {text}"));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse ComicVine response: {e}"))?;

    let results = match data.get("results").and_then(|r| r.as_array()) {
        Some(results) => results,
        None => return Ok(vec![]),
    };

    let books: Vec<BookCandidate> = results
        .iter()
        .filter_map(|issue| {
            let id = issue.get("id").and_then(|id| id.as_i64())?;
            let name = issue
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let issue_number = issue
                .get("issue_number")
                .and_then(|n| n.as_str())
                .and_then(|n| n.parse::<f64>().ok())
                .map(|n| n as i32);
            let description = issue
                .get("description")
                .and_then(|d| d.as_str())
                .map(strip_html);
            let cover_url = issue
                .get("image")
                .and_then(|img| img.get("medium_url").or_else(|| img.get("small_url")))
                .and_then(|u| u.as_str())
                .map(String::from);
            let cover_date = issue
                .get("cover_date")
                .and_then(|d| d.as_str())
                .map(String::from);

            Some(BookCandidate {
                external_book_id: id.to_string(),
                title: name,
                volume_number: issue_number,
                authors: vec![],
                isbn: None,
                summary: description,
                cover_url,
                page_count: None,
                language: None,
                publish_date: cover_date,
                metadata_json: serde_json::json!({}),
            })
        })
        .collect();

    Ok(books)
}

fn strip_html(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
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

fn urlencoded(s: &str) -> String {
    let mut result = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => result.push_str(&format!("%{:02X}", byte)),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_config() -> ProviderConfig {
        ProviderConfig {
            api_key: Some("test_key".to_string()),
            language: "en".to_string(),
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
        assert_eq!(best.description, Some("A noir detective story.".to_string()));

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

        assert!(candidates.is_empty(), "should return empty vec for no results");
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
                    "site_detail_url": "https://comicvine.example.com/issue/100/"
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
        assert!(books[0].authors.is_empty());
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
        };

        let result = get_series_books_impl("12345", &config, "http://unused").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key"));
    }
}
