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

            let mut metadata_json = serde_json::json!({});
            if let Some(ref desc) = description {
                metadata_json["description"] = serde_json::json!(desc);
            }
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
                metadata_json,
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
#[path = "tests/comicvine.rs"]
mod tests;
