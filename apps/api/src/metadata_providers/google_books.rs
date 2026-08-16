use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const DEFAULT_BASE_URL: &str = "https://www.googleapis.com";

pub struct GoogleBooksProvider;

impl MetadataProvider for GoogleBooksProvider {
    fn name(&self) -> &str {
        "google_books"
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

    fn get_series(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<SeriesCandidate, String>> + Send + '_>,
    > {
        let external_id = external_id.to_string();
        let config = config.clone();
        Box::pin(async move { get_series_impl(&external_id, &config, DEFAULT_BASE_URL).await })
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
        Box::pin(
            async move { get_series_books_impl(&external_id, &config, DEFAULT_BASE_URL).await },
        )
    }
}

async fn search_series_impl(
    query: &str,
    config: &ProviderConfig,
    base_url: &str,
) -> Result<Vec<SeriesCandidate>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let search_query = format!("intitle:{}", query);
    let mut url = format!(
        "{}/books/v1/volumes?q={}&maxResults=20&printType=books&langRestrict={}",
        base_url,
        urlencoded(&search_query),
        urlencoded(&config.language),
    );
    if let Some(ref key) = config.api_key {
        url.push_str(&format!("&key={}", key));
    }

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Google Books request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Google Books returned {status}: {text}"));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google Books response: {e}"))?;

    let items = match data.get("items").and_then(|i| i.as_array()) {
        Some(items) => items,
        None => return Ok(vec![]),
    };

    // Group volumes by series name to produce series candidates
    let query_lower = query.to_lowercase();
    let mut series_map: std::collections::HashMap<String, SeriesCandidateBuilder> =
        std::collections::HashMap::new();

    for item in items {
        let volume_info = match item.get("volumeInfo") {
            Some(vi) => vi,
            None => continue,
        };

        let title = volume_info
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let authors: Vec<String> = volume_info
            .get("authors")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let publisher = volume_info
            .get("publisher")
            .and_then(|p| p.as_str())
            .map(String::from);
        let published_date = volume_info
            .get("publishedDate")
            .and_then(|d| d.as_str())
            .map(String::from);
        let description = volume_info
            .get("description")
            .and_then(|d| d.as_str())
            .map(String::from);

        // Extract series info from title or seriesInfo
        let series_name = volume_info
            .get("seriesInfo")
            .and_then(|si| si.get("title"))
            .and_then(|t| t.as_str())
            .map(String::from)
            .unwrap_or_else(|| extract_series_name(&title));

        let cover_url = volume_info
            .get("imageLinks")
            .and_then(|il| il.get("thumbnail").or_else(|| il.get("smallThumbnail")))
            .and_then(|u| u.as_str())
            .map(|s| s.replace("http://", "https://"));

        let google_id = item
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or("")
            .to_string();

        let avg_rating = volume_info.get("averageRating").and_then(|r| r.as_f64());
        let ratings_count = volume_info.get("ratingsCount").and_then(|c| c.as_i64());

        let entry =
            series_map
                .entry(series_name.clone())
                .or_insert_with(|| SeriesCandidateBuilder {
                    title: series_name.clone(),
                    authors: vec![],
                    description: None,
                    publishers: vec![],
                    start_year: None,
                    volume_count: 0,
                    cover_url: None,
                    external_id: google_id.clone(),
                    external_url: None,
                    metadata_json: serde_json::json!({}),
                    rating: None,
                    rating_count: None,
                });

        entry.volume_count += 1;

        // Keep the rating from the volume with the highest ratings_count
        if ratings_count.unwrap_or(0) > entry.rating_count.unwrap_or(0) {
            entry.rating = avg_rating;
            entry.rating_count = ratings_count;
        }

        // Merge authors
        for a in &authors {
            if !entry.authors.contains(a) {
                entry.authors.push(a.clone());
            }
        }

        // Set description if not yet set
        if entry.description.is_none() {
            entry.description = description;
        }

        // Merge publisher
        if let Some(ref pub_name) = publisher {
            if !entry.publishers.contains(pub_name) {
                entry.publishers.push(pub_name.clone());
            }
        }

        // Extract year
        if let Some(ref date) = published_date {
            if let Some(year) = extract_year(date) {
                if entry.start_year.is_none() || entry.start_year.unwrap() > year {
                    entry.start_year = Some(year);
                }
            }
        }

        if entry.cover_url.is_none() {
            entry.cover_url = cover_url;
        }

        entry.external_url = Some(format!("https://books.google.com/books?id={}", google_id));
    }

    let mut candidates: Vec<SeriesCandidate> = series_map
        .into_values()
        .map(|b| {
            let confidence = compute_confidence(&b.title, &query_lower);
            let mut metadata_json = b.metadata_json;
            if let Some(ref desc) = b.description {
                metadata_json["description"] = serde_json::json!(desc);
            }
            if let Some(r) = b.rating {
                metadata_json["rating"] = serde_json::json!(r);
                metadata_json["rating_scale"] = serde_json::json!(5.0_f64);
            }
            if let Some(c) = b.rating_count {
                metadata_json["rating_count"] = serde_json::json!(c);
            }
            SeriesCandidate {
                external_id: b.external_id,
                title: b.title,
                authors: b.authors,
                description: b.description,
                publishers: b.publishers,
                start_year: b.start_year,
                total_volumes: if b.volume_count > 1 {
                    Some(b.volume_count)
                } else {
                    None
                },
                cover_url: b.cover_url,
                external_url: b.external_url,
                confidence,
                metadata_json,
            }
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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;
    let mut url = format!("{base_url}/books/v1/volumes/{external_id}");
    if let Some(key) = &config.api_key {
        url.push_str(&format!("?key={key}"));
    }
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Google Books request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Google Books returned {}", response.status()));
    }
    let volume: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google Books response: {e}"))?;
    let info = volume
        .get("volumeInfo")
        .ok_or_else(|| format!("Google Books volume {external_id} has no volumeInfo"))?;
    let title = info
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let authors = info
        .get("authors")
        .and_then(|value| value.as_array())
        .map(|authors| {
            authors
                .iter()
                .filter_map(|author| author.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let description = info
        .get("description")
        .and_then(|value| value.as_str())
        .map(String::from);
    let publishers = info
        .get("publisher")
        .and_then(|value| value.as_str())
        .map(String::from)
        .into_iter()
        .collect();
    let start_year = info
        .get("publishedDate")
        .and_then(|value| value.as_str())
        .and_then(extract_year);
    let cover_url = info
        .get("imageLinks")
        .and_then(|links| {
            links
                .get("thumbnail")
                .or_else(|| links.get("smallThumbnail"))
        })
        .and_then(|value| value.as_str())
        .map(|url| url.replace("http://", "https://"));
    let mut metadata_json = serde_json::json!({});
    if let Some(description) = &description {
        metadata_json["description"] = serde_json::json!(description);
    }
    if let Some(rating) = info.get("averageRating").and_then(|value| value.as_f64()) {
        metadata_json["rating"] = serde_json::json!(rating);
        metadata_json["rating_scale"] = serde_json::json!(5.0);
    }
    if let Some(count) = info.get("ratingsCount").and_then(|value| value.as_i64()) {
        metadata_json["rating_count"] = serde_json::json!(count);
    }
    Ok(SeriesCandidate {
        external_id: external_id.to_string(),
        title,
        authors,
        description,
        publishers,
        start_year,
        total_volumes: None,
        cover_url,
        external_url: Some(format!("https://books.google.com/books?id={external_id}")),
        confidence: 1.0,
        metadata_json,
    })
}

async fn get_series_books_impl(
    external_id: &str,
    config: &ProviderConfig,
    base_url: &str,
) -> Result<Vec<BookCandidate>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    // First fetch the volume to get its series info
    let mut url = format!("{}/books/v1/volumes/{}", base_url, external_id);
    if let Some(ref key) = config.api_key {
        url.push_str(&format!("?key={}", key));
    }

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Google Books request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Google Books returned {status}: {text}"));
    }

    let volume: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google Books response: {e}"))?;

    let volume_info = volume
        .get("volumeInfo")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let title = volume_info
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Search for more volumes in this series
    let series_name = extract_series_name(title);
    let search_query = format!("intitle:{}", series_name);
    let mut search_url = format!(
        "{}/books/v1/volumes?q={}&maxResults=40&printType=books&langRestrict={}",
        base_url,
        urlencoded(&search_query),
        urlencoded(&config.language),
    );
    if let Some(ref key) = config.api_key {
        search_url.push_str(&format!("&key={}", key));
    }

    let resp = client
        .get(&search_url)
        .send()
        .await
        .map_err(|e| format!("Google Books search failed: {e}"))?;

    if !resp.status().is_success() {
        // Return just the single volume as a book
        return Ok(vec![volume_to_book_candidate(&volume)]);
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse search response: {e}"))?;

    let items = match data.get("items").and_then(|i| i.as_array()) {
        Some(items) => items,
        None => return Ok(vec![volume_to_book_candidate(&volume)]),
    };

    let mut books: Vec<BookCandidate> = items.iter().map(volume_to_book_candidate).collect();

    // Sort by volume number
    books.sort_by_key(|b| b.volume_number.unwrap_or(999));

    Ok(books)
}

fn volume_to_book_candidate(item: &serde_json::Value) -> BookCandidate {
    let volume_info = item
        .get("volumeInfo")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let title = volume_info
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let authors: Vec<String> = volume_info
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let isbn = volume_info
        .get("industryIdentifiers")
        .and_then(|ids| ids.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|id| {
                    id.get("type")
                        .and_then(|t| t.as_str())
                        .map(|t| t == "ISBN_13" || t == "ISBN_10")
                        .unwrap_or(false)
                })
                .and_then(|id| id.get("identifier").and_then(|i| i.as_str()))
        })
        .map(String::from);
    let summary = volume_info
        .get("description")
        .and_then(|d| d.as_str())
        .map(String::from);
    let cover_url = volume_info
        .get("imageLinks")
        .and_then(|il| il.get("thumbnail").or_else(|| il.get("smallThumbnail")))
        .and_then(|u| u.as_str())
        .map(|s| s.replace("http://", "https://"));
    let page_count = volume_info
        .get("pageCount")
        .and_then(|p| p.as_i64())
        .map(|p| p as i32);
    let language = volume_info
        .get("language")
        .and_then(|l| l.as_str())
        .map(String::from);
    let publish_date = volume_info
        .get("publishedDate")
        .and_then(|d| d.as_str())
        .map(String::from);
    let google_id = item
        .get("id")
        .and_then(|id| id.as_str())
        .unwrap_or("")
        .to_string();
    let volume_number =
        parsers::extract_metadata_volume(&title).or_else(|| extract_fallback_volume_number(&title));

    BookCandidate {
        external_book_id: google_id,
        title,
        volume_number,
        authors,
        isbn,
        summary,
        cover_url,
        page_count,
        language,
        publish_date,
        metadata_json: serde_json::json!({}),
    }
}

fn extract_series_name(title: &str) -> String {
    // Remove trailing volume indicators like "Vol. 1", "Tome 2", "#3", "- Volume 1"
    let re_patterns = [
        r"(?i)\s*[-–—]\s*(?:vol(?:ume)?\.?\s*|tome\s*|t\.\s*|#)\s*\d+.*$",
        r"(?i)\s*,?\s*(?:vol(?:ume)?\.?\s*|tome\s*|t\.\s*|#)\s*\d+.*$",
        r"\s*\(\d+\)\s*$",
        r"\s+\d+\s*$",
    ];

    let mut result = title.to_string();
    for pattern in &re_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            let cleaned = re.replace(&result, "").to_string();
            if !cleaned.is_empty() && cleaned != result {
                result = cleaned;
                break;
            }
        }
    }

    result.trim().to_string()
}

fn extract_fallback_volume_number(title: &str) -> Option<i32> {
    let patterns = [r"\((\d+)\)\s*$", r"\b(\d+)\s*$"];

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(title) {
                if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                    return Some(num);
                }
            }
        }
    }

    None
}

fn extract_year(date: &str) -> Option<i32> {
    date.get(..4).and_then(|s| s.parse::<i32>().ok())
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
        // Simple character overlap ratio
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
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

struct SeriesCandidateBuilder {
    title: String,
    authors: Vec<String>,
    description: Option<String>,
    publishers: Vec<String>,
    start_year: Option<i32>,
    volume_count: i32,
    cover_url: Option<String>,
    external_id: String,
    external_url: Option<String>,
    metadata_json: serde_json::Value,
    rating: Option<f64>,
    rating_count: Option<i64>,
}

#[cfg(test)]
#[path = "tests/google_books.rs"]
mod tests;
