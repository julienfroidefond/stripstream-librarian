use scraper::{Html, Selector};

use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

pub struct BedethequeProvider;

impl MetadataProvider for BedethequeProvider {
    fn name(&self) -> &str {
        "bedetheque"
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

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:108.0) Gecko/20100101 Firefox/108.0")
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                reqwest::header::ACCEPT,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                    .parse()
                    .unwrap(),
            );
            h.insert(
                reqwest::header::ACCEPT_LANGUAGE,
                "fr-FR,fr;q=0.9,en;q=0.5".parse().unwrap(),
            );
            h.insert(reqwest::header::REFERER, "https://www.bedetheque.com/".parse().unwrap());
            h
        })
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

/// Remove diacritics for URL construction (bedetheque uses ASCII slugs)
fn normalize_for_url(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'à' | 'â' | 'ä' | 'À' | 'Â' | 'Ä' => 'a',
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ô' | 'ö' | 'Ô' | 'Ö' => 'o',
            'î' | 'ï' | 'Î' | 'Ï' => 'i',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            _ => c,
        })
        .collect()
}

fn urlencoded(s: &str) -> String {
    let mut result = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push('+'),
            _ => result.push_str(&format!("%{:02X}", byte)),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

async fn search_series_impl(
    query: &str,
    _config: &ProviderConfig,
) -> Result<Vec<SeriesCandidate>, String> {
    let client = build_client()?;

    // Use the full-text search page
    let url = format!(
        "https://www.bedetheque.com/search/tout?RechTexte={}&RechWhere=0",
        urlencoded(&normalize_for_url(query))
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Bedetheque request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!("Bedetheque returned {status}"));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read Bedetheque response: {e}"))?;

    // Detect IP blacklist
    if html.contains("<title></title>") || html.contains("<title> </title>") {
        return Err("Bedetheque: IP may be rate-limited, please retry later".to_string());
    }

    // Parse HTML in a block so the non-Send Html type is dropped before any .await
    let candidates = {
        let document = Html::parse_document(&html);
        let link_sel =
            Selector::parse("a[href*='/serie-']").map_err(|e| format!("selector error: {e}"))?;

        let query_lower = query.to_lowercase();
        let mut seen = std::collections::HashSet::new();
        let mut candidates = Vec::new();

        for el in document.select(&link_sel) {
            let href = match el.value().attr("href") {
                Some(h) => h.to_string(),
                None => continue,
            };

            let (series_id, _slug) = match parse_serie_href(&href) {
                Some(v) => v,
                None => continue,
            };

            if !seen.insert(series_id.clone()) {
                continue;
            }

            let title = el.text().collect::<String>().trim().to_string();
            if title.is_empty() {
                continue;
            }

            let confidence = compute_confidence(&title, &query_lower);
            let cover_url = format!(
                "https://www.bedetheque.com/cache/thb_series/PlancheS_{}.jpg",
                series_id
            );

            candidates.push(SeriesCandidate {
                external_id: series_id.clone(),
                title: title.clone(),
                authors: vec![],
                description: None,
                publishers: vec![],
                start_year: None,
                total_volumes: None,
                cover_url: Some(cover_url),
                external_url: Some(href),
                confidence,
                metadata_json: serde_json::json!({}),
            });
        }

        candidates.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(10);
        candidates
    }; // document is dropped here — safe to .await below

    // For the top candidates, fetch series details to enrich metadata
    // (limit to top 3 to avoid hammering the site)
    let mut enriched = Vec::new();
    for mut c in candidates {
        if enriched.len() < 3 {
            if let Ok(details) = fetch_series_details(&client, &c.external_id, c.external_url.as_deref()).await {
                if let Some(desc) = details.description {
                    c.description = Some(desc);
                }
                if !details.authors.is_empty() {
                    c.authors = details.authors;
                }
                if !details.publishers.is_empty() {
                    c.publishers = details.publishers;
                }
                if let Some(year) = details.start_year {
                    c.start_year = Some(year);
                }
                if let Some(count) = details.album_count {
                    c.total_volumes = Some(count);
                }
                c.metadata_json = serde_json::json!({
                    "description": c.description,
                    "authors": c.authors,
                    "publishers": c.publishers,
                    "start_year": c.start_year,
                });
            }
        }
        enriched.push(c);
    }

    Ok(enriched)
}

/// Parse serie URL to extract (id, slug)
fn parse_serie_href(href: &str) -> Option<(String, String)> {
    // Patterns:
    //   https://www.bedetheque.com/serie-3-BD-Blacksad.html
    //   /serie-3-BD-Blacksad.html
    let re = regex::Regex::new(r"/serie-(\d+)-[A-Za-z]+-(.+?)(?:__\d+)?\.html").ok()?;
    let caps = re.captures(href)?;
    Some((caps[1].to_string(), caps[2].to_string()))
}

struct SeriesDetails {
    description: Option<String>,
    authors: Vec<String>,
    publishers: Vec<String>,
    start_year: Option<i32>,
    album_count: Option<i32>,
}

async fn fetch_series_details(
    client: &reqwest::Client,
    series_id: &str,
    series_url: Option<&str>,
) -> Result<SeriesDetails, String> {
    // Build URL — append __10000 to get all albums on one page
    let url = match series_url {
        Some(u) => {
            // Replace .html with __10000.html
            u.replace(".html", "__10000.html")
        }
        None => format!(
            "https://www.bedetheque.com/serie-{}-BD-Serie__10000.html",
            series_id
        ),
    };

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch series page: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Series page returned {}", resp.status()));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read series page: {e}"))?;

    let doc = Html::parse_document(&html);
    let mut details = SeriesDetails {
        description: None,
        authors: vec![],
        publishers: vec![],
        start_year: None,
        album_count: None,
    };

    // Description: look for #full-commentaire or .serie-info
    if let Ok(sel) = Selector::parse("#full-commentaire") {
        if let Some(el) = doc.select(&sel).next() {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                details.description = Some(text);
            }
        }
    }

    // Fallback description from span.infoedition
    if details.description.is_none() {
        if let Ok(sel) = Selector::parse("span.infoedition") {
            if let Some(el) = doc.select(&sel).next() {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    details.description = Some(text);
                }
            }
        }
    }

    // Extract authors and publishers from album info blocks
    if let Ok(sel) = Selector::parse(".infos li") {
        let mut authors_set = std::collections::HashSet::new();
        let mut publishers_set = std::collections::HashSet::new();

        for li in doc.select(&sel) {
            let text = li.text().collect::<String>();
            let text = text.trim();

            if let Some(val) = extract_info_value(text, "Scénario") {
                for a in val.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    authors_set.insert(a.to_string());
                }
            }
            if let Some(val) = extract_info_value(text, "Dessin") {
                for a in val.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    authors_set.insert(a.to_string());
                }
            }
            if let Some(val) = extract_info_value(text, "Editeur") {
                for p in val.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    publishers_set.insert(p.to_string());
                }
            }
        }

        details.authors = authors_set.into_iter().collect();
        details.authors.sort();
        details.publishers = publishers_set.into_iter().collect();
        details.publishers.sort();
    }

    // Album count from serie-info text (e.g. "Tomes : 8")
    let page_text = doc.root_element().text().collect::<String>();
    if let Ok(re) = regex::Regex::new(r"Tomes?\s*:\s*(\d+)") {
        if let Some(caps) = re.captures(&page_text) {
            if let Ok(n) = caps[1].parse::<i32>() {
                details.album_count = Some(n);
            }
        }
    }

    // Start year from first album date (Dépot légal)
    if let Ok(re) = regex::Regex::new(r"[Dd][ée]p[ôo]t l[ée]gal\s*:\s*\d{2}/(\d{4})") {
        if let Some(caps) = re.captures(&page_text) {
            if let Ok(year) = caps[1].parse::<i32>() {
                details.start_year = Some(year);
            }
        }
    }

    Ok(details)
}

/// Extract value after a label like "Scénario : Jean-Claude" → "Jean-Claude"
fn extract_info_value<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    // Handle both "Label :" and "Label:"
    let patterns = [
        format!("{} :", label),
        format!("{}:", label),
        format!("{} :", &label.to_lowercase()),
    ];
    for pat in &patterns {
        if let Some(pos) = text.find(pat.as_str()) {
            let val = text[pos + pat.len()..].trim();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Get series books
// ---------------------------------------------------------------------------

async fn get_series_books_impl(
    external_id: &str,
    _config: &ProviderConfig,
) -> Result<Vec<BookCandidate>, String> {
    let client = build_client()?;

    // We need to find the series URL — try a direct fetch
    // external_id is the numeric series ID
    // We try to fetch the series page to get the album list
    let url = format!(
        "https://www.bedetheque.com/serie-{}-BD-Serie__10000.html",
        external_id
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch series: {e}"))?;

    // If the generic slug fails, try without the slug part (bedetheque redirects)
    let html = if resp.status().is_success() {
        resp.text().await.map_err(|e| format!("Failed to read: {e}"))?
    } else {
        // Try alternative URL pattern
        let alt_url = format!(
            "https://www.bedetheque.com/serie-{}__10000.html",
            external_id
        );
        let resp2 = client
            .get(&alt_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch series (alt): {e}"))?;
        if !resp2.status().is_success() {
            return Err(format!("Series page not found for id {external_id}"));
        }
        resp2.text().await.map_err(|e| format!("Failed to read: {e}"))?
    };

    if html.contains("<title></title>") {
        return Err("Bedetheque: IP may be rate-limited".to_string());
    }

    let doc = Html::parse_document(&html);
    let mut books = Vec::new();

    // Albums are in .album-main blocks
    let album_sel = Selector::parse(".album-main").map_err(|e| format!("selector: {e}"))?;

    for album_el in doc.select(&album_sel) {
        let album_html = album_el.html();
        let album_doc = Html::parse_fragment(&album_html);

        // Title from .titre
        let title = select_text(&album_doc, ".titre")
            .or_else(|| {
                Selector::parse(".titre a")
                    .ok()
                    .and_then(|s| album_doc.select(&s).next())
                    .map(|el| el.text().collect::<String>().trim().to_string())
            })
            .unwrap_or_default();

        if title.is_empty() {
            continue;
        }

        // Volume number from title or .num span
        let volume_number = select_text(&album_doc, ".num")
            .and_then(|s| {
                s.trim_end_matches('.')
                    .trim()
                    .parse::<i32>()
                    .ok()
            })
            .or_else(|| extract_volume_from_title(&title));

        // Album URL
        let album_url = Selector::parse("a[href*='/BD-']")
            .ok()
            .and_then(|s| album_doc.select(&s).next())
            .and_then(|el| el.value().attr("href"))
            .map(String::from);

        // External book id from URL
        let external_book_id = album_url
            .as_deref()
            .and_then(|u| {
                regex::Regex::new(r"-(\d+)\.html")
                    .ok()
                    .and_then(|re| re.captures(u))
                    .map(|c| c[1].to_string())
            })
            .unwrap_or_default();

        // Cover
        let cover_url = Selector::parse("img[src*='cache/thb_couv']")
            .ok()
            .and_then(|s| album_doc.select(&s).next())
            .and_then(|el| el.value().attr("src"))
            .map(|s| {
                if s.starts_with("http") {
                    s.to_string()
                } else {
                    format!("https://www.bedetheque.com{}", s)
                }
            });

        // Extract info fields
        let album_text = album_el.text().collect::<String>();
        let authors = extract_all_authors(&album_text);
        let isbn = extract_info_value(&album_text, "EAN/ISBN")
            .or_else(|| extract_info_value(&album_text, "ISBN"))
            .map(|s| s.trim().to_string());
        let page_count = extract_info_value(&album_text, "Planches")
            .and_then(|s| s.trim().parse::<i32>().ok());
        let publish_date = extract_info_value(&album_text, "Dépot légal")
            .or_else(|| extract_info_value(&album_text, "Depot legal"))
            .map(|s| s.trim().to_string());

        books.push(BookCandidate {
            external_book_id,
            title,
            volume_number,
            authors,
            isbn,
            summary: None,
            cover_url,
            page_count,
            language: Some("fr".to_string()),
            publish_date,
            metadata_json: serde_json::json!({}),
        });
    }

    books.sort_by_key(|b| b.volume_number.unwrap_or(999));
    Ok(books)
}

fn select_text(doc: &Html, selector: &str) -> Option<String> {
    Selector::parse(selector)
        .ok()
        .and_then(|s| doc.select(&s).next())
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_all_authors(text: &str) -> Vec<String> {
    let mut authors = Vec::new();
    for label in ["Scénario", "Scenario", "Dessin"] {
        if let Some(val) = extract_info_value(text, label) {
            for a in val.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if !authors.contains(&a.to_string()) {
                    authors.push(a.to_string());
                }
            }
        }
    }
    authors
}

fn extract_volume_from_title(title: &str) -> Option<i32> {
    let patterns = [
        r"(?i)(?:tome|t\.)\s*(\d+)",
        r"(?i)(?:vol(?:ume)?\.?)\s*(\d+)",
        r"#\s*(\d+)",
    ];
    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(title) {
                if let Ok(n) = caps[1].parse::<i32>() {
                    return Some(n);
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
        0.85
    } else if title_lower.contains(query) || query.contains(&title_lower) {
        0.7
    } else {
        let common: usize = query
            .chars()
            .filter(|c| title_lower.contains(*c))
            .count();
        let max_len = query.len().max(title_lower.len()).max(1);
        (common as f32 / max_len as f32).clamp(0.1, 0.6)
    }
}
