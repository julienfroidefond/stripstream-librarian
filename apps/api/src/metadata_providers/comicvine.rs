use stripstream_core::http::build_http_client_with_agent;

use super::{
    compute_confidence, strip_html, urlencoded, BookCandidate, MetadataProvider, ProviderConfig,
    SeriesCandidate,
};

const DEFAULT_BASE_URL: &str = "https://comicvine.gamespot.com";

pub struct ComicVineProvider;

#[async_trait::async_trait]
impl MetadataProvider for ComicVineProvider {
    fn name(&self) -> &str {
        "comicvine"
    }

    async fn search_series(
        &self,
        query: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<SeriesCandidate>, String> {
        search_series_impl(query, config, DEFAULT_BASE_URL).await
    }

    async fn get_series(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> Result<SeriesCandidate, String> {
        get_series_impl(external_id, config, DEFAULT_BASE_URL).await
    }

    async fn get_series_books(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<BookCandidate>, String> {
        get_series_books_impl(external_id, config, DEFAULT_BASE_URL).await
    }
}

fn build_client() -> Result<reqwest::Client, String> {
    build_http_client_with_agent(std::time::Duration::from_secs(15))
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
        .ok_or_else(|| {
            "ComicVine requires an API key. Configure it in Settings > Integrations.".to_string()
        })?;

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

    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.truncate(10);
    Ok(candidates)
}

async fn get_series_impl(
    external_id: &str,
    config: &ProviderConfig,
    base_url: &str,
) -> Result<SeriesCandidate, String> {
    let api_key = config
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| "ComicVine requires an API key".to_string())?;
    let client = build_client()?;
    let url = format!("{base_url}/api/volume/4050-{external_id}/?api_key={api_key}&format=json");
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ComicVine request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("ComicVine returned {}", response.status()));
    }
    let volume: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse ComicVine response: {e}"))?;
    let volume = volume
        .get("results")
        .ok_or_else(|| format!("ComicVine volume {external_id} not found"))?;
    let title = volume
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("ComicVine volume {external_id} has no name"))?
        .to_string();
    let description = volume
        .get("description")
        .and_then(|value| value.as_str())
        .map(strip_html);
    let publishers = volume
        .get("publisher")
        .and_then(|value| value.get("name"))
        .and_then(|value| value.as_str())
        .map(String::from)
        .into_iter()
        .collect();
    let start_year = volume
        .get("start_year")
        .and_then(|value| value.as_str())
        .and_then(|year| year.parse().ok());
    let total_volumes = volume
        .get("count_of_issues")
        .and_then(|value| value.as_i64())
        .map(|count| count as i32);
    let cover_url = volume
        .get("image")
        .and_then(|image| image.get("medium_url").or_else(|| image.get("small_url")))
        .and_then(|value| value.as_str())
        .map(String::from);
    let external_url = volume
        .get("site_detail_url")
        .and_then(|value| value.as_str())
        .map(String::from);
    let mut metadata_json = serde_json::json!({});
    if let Some(description) = &description {
        metadata_json["description"] = serde_json::json!(description);
    }

    Ok(SeriesCandidate {
        external_id: external_id.to_string(),
        title,
        authors: vec![],
        description,
        publishers,
        start_year,
        total_volumes,
        cover_url,
        external_url,
        confidence: 1.0,
        metadata_json,
    })
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
        "{}/api/issues/?api_key={}&format=json&filter=volume:{}&sort=issue_number:asc&limit=100&field_list=id,name,issue_number,description,image,cover_date,site_detail_url,person_credits",
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
            let authors = issue
                .get("person_credits")
                .and_then(|credits| credits.as_array())
                .map(|credits| {
                    credits
                        .iter()
                        .filter_map(|credit| credit.get("name").and_then(|n| n.as_str()))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();

            Some(BookCandidate {
                external_book_id: id.to_string(),
                title: name,
                volume_number: issue_number,
                authors,
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

#[cfg(test)]
#[path = "tests/comicvine.rs"]
mod tests;
