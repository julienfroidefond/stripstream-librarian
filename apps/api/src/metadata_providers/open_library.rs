use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const DEFAULT_BASE_URL: &str = "https://openlibrary.org";

pub struct OpenLibraryProvider;

impl MetadataProvider for OpenLibraryProvider {
    fn name(&self) -> &str {
        "open_library"
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

    // Open Library uses 3-letter language codes
    let ol_lang = match config.language.as_str() {
        "fr" => "fre",
        "es" => "spa",
        _ => "eng",
    };

    let url = format!(
        "{}/search.json?title={}&limit=20&language={}",
        base_url,
        urlencoded(query),
        ol_lang,
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Open Library request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Open Library returned {status}: {text}"));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Open Library response: {e}"))?;

    let docs = match data.get("docs").and_then(|d| d.as_array()) {
        Some(docs) => docs,
        None => return Ok(vec![]),
    };

    let query_lower = query.to_lowercase();
    let mut series_map: std::collections::HashMap<String, SeriesCandidateBuilder> =
        std::collections::HashMap::new();

    for doc in docs {
        let title = doc
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let authors: Vec<String> = doc
            .get("author_name")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let publishers: Vec<String> = doc
            .get("publisher")
            .and_then(|a| a.as_array())
            .map(|arr| {
                let mut pubs: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                pubs.truncate(3);
                pubs
            })
            .unwrap_or_default();
        let first_publish_year = doc
            .get("first_publish_year")
            .and_then(|y| y.as_i64())
            .map(|y| y as i32);
        let cover_i = doc.get("cover_i").and_then(|c| c.as_i64());
        let cover_url =
            cover_i.map(|id| format!("https://covers.openlibrary.org/b/id/{}-M.jpg", id));
        let key = doc
            .get("key")
            .and_then(|k| k.as_str())
            .unwrap_or("")
            .to_string();

        let ratings_average = doc.get("ratings_average").and_then(|r| r.as_f64());
        let ratings_count = doc.get("ratings_count").and_then(|c| c.as_i64());

        let series_name = extract_series_name(&title);

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
                    external_id: key.clone(),
                    external_url: if key.is_empty() {
                        None
                    } else {
                        Some(format!("https://openlibrary.org{}", key))
                    },
                    rating: None,
                    rating_count: None,
                });

        entry.volume_count += 1;

        // Keep the rating from the doc with the highest ratings_count
        if ratings_count.unwrap_or(0) > entry.rating_count.unwrap_or(0) {
            entry.rating = ratings_average;
            entry.rating_count = ratings_count;
        }

        for a in &authors {
            if !entry.authors.contains(a) {
                entry.authors.push(a.clone());
            }
        }
        for p in &publishers {
            if !entry.publishers.contains(p) {
                entry.publishers.push(p.clone());
            }
        }
        if (entry.start_year.is_none()
            || first_publish_year.is_some_and(|y| entry.start_year.unwrap() > y))
            && first_publish_year.is_some()
        {
            entry.start_year = first_publish_year;
        }
        if entry.cover_url.is_none() {
            entry.cover_url = cover_url;
        }
    }

    let mut candidates: Vec<SeriesCandidate> = series_map
        .into_values()
        .map(|b| {
            let confidence = compute_confidence(&b.title, &query_lower);
            let mut metadata_json = serde_json::json!({});
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

async fn get_series_books_impl(
    external_id: &str,
    _config: &ProviderConfig,
    base_url: &str,
) -> Result<Vec<BookCandidate>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    // Fetch the work to get its title for series search
    let url = format!("{}{}.json", base_url, external_id);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Open Library request failed: {e}"))?;

    let work: serde_json::Value = if resp.status().is_success() {
        resp.json()
            .await
            .map_err(|e| format!("Failed to parse response: {e}"))?
    } else {
        serde_json::json!({})
    };

    let title = work.get("title").and_then(|t| t.as_str()).unwrap_or("");
    let series_name = extract_series_name(title);

    // Search for editions of this series
    let search_url = format!(
        "{}/search.json?title={}&limit=40",
        base_url,
        urlencoded(&series_name)
    );
    let resp = client
        .get(&search_url)
        .send()
        .await
        .map_err(|e| format!("Open Library search failed: {e}"))?;

    if !resp.status().is_success() {
        return Ok(vec![]);
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;
    let docs = match data.get("docs").and_then(|d| d.as_array()) {
        Some(docs) => docs,
        None => return Ok(vec![]),
    };

    let mut books: Vec<BookCandidate> = docs
        .iter()
        .map(|doc| {
            let title = doc
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let authors: Vec<String> = doc
                .get("author_name")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let isbn = doc
                .get("isbn")
                .and_then(|a| a.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(String::from);
            let page_count = doc
                .get("number_of_pages_median")
                .and_then(|n| n.as_i64())
                .map(|n| n as i32);
            let cover_i = doc.get("cover_i").and_then(|c| c.as_i64());
            let cover_url =
                cover_i.map(|id| format!("https://covers.openlibrary.org/b/id/{}-M.jpg", id));
            let language = doc
                .get("language")
                .and_then(|a| a.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(String::from);
            let publish_date = doc
                .get("first_publish_year")
                .and_then(|y| y.as_i64())
                .map(|y| y.to_string());
            let key = doc
                .get("key")
                .and_then(|k| k.as_str())
                .unwrap_or("")
                .to_string();
            let volume_number = extract_volume_number(&title);

            BookCandidate {
                external_book_id: key,
                title,
                volume_number,
                authors,
                isbn,
                summary: None,
                cover_url,
                page_count,
                language,
                publish_date,
                metadata_json: serde_json::json!({}),
            }
        })
        .collect();

    books.sort_by_key(|b| b.volume_number.unwrap_or(999));
    Ok(books)
}

fn extract_series_name(title: &str) -> String {
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

fn extract_volume_number(title: &str) -> Option<i32> {
    let patterns = [
        r"(?i)(?:vol(?:ume)?\.?\s*|tome\s*|t\.\s*|#)\s*(\d+)",
        r"\((\d+)\)\s*$",
        r"\b(\d+)\s*$",
    ];
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
    rating: Option<f64>,
    rating_count: Option<i64>,
}

#[cfg(test)]
#[path = "tests/open_library.rs"]
mod tests;
