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
                    "genres": details.genres,
                    "status": details.status,
                    "origin": details.origin,
                    "language": details.language,
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
    genres: Vec<String>,
    status: Option<String>,
    origin: Option<String>,
    language: Option<String>,
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
        genres: vec![],
        status: None,
        origin: None,
        language: None,
    };

    // Description from <meta name="description"> — format: "Tout sur la série {name} : {description}"
    if let Ok(sel) = Selector::parse(r#"meta[name="description"]"#) {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let desc = content.trim().to_string();
                // Strip the "Tout sur la série ... : " prefix
                let cleaned = if let Some(pos) = desc.find(" : ") {
                    desc[pos + 3..].trim().to_string()
                } else {
                    desc
                };
                if !cleaned.is_empty() {
                    details.description = Some(cleaned);
                }
            }
        }
    }

    // Extract authors from itemprop="author" and itemprop="illustrator" (deduplicated)
    {
        let mut authors_set = std::collections::HashSet::new();
        for attr in ["author", "illustrator"] {
            if let Ok(sel) = Selector::parse(&format!(r#"[itemprop="{attr}"]"#)) {
                for el in doc.select(&sel) {
                    let name = el.text().collect::<String>().trim().to_string();
                    // Names are "Last, First" — normalize to "First Last"
                    let normalized = if let Some((last, first)) = name.split_once(',') {
                        format!("{} {}", first.trim(), last.trim())
                    } else {
                        name
                    };
                    if !normalized.is_empty() && is_real_author(&normalized) {
                        authors_set.insert(normalized);
                    }
                }
            }
        }
        details.authors = authors_set.into_iter().collect();
        details.authors.sort();
    }

    // Extract publishers from itemprop="publisher" (deduplicated)
    {
        let mut publishers_set = std::collections::HashSet::new();
        if let Ok(sel) = Selector::parse(r#"[itemprop="publisher"]"#) {
            for el in doc.select(&sel) {
                let name = el.text().collect::<String>().trim().to_string();
                if !name.is_empty() {
                    publishers_set.insert(name);
                }
            }
        }
        details.publishers = publishers_set.into_iter().collect();
        details.publishers.sort();
    }

    // Extract series-level info from <li><label>X :</label>value</li> blocks
    // Genre: <li><label>Genre :</label><span class="style-serie">Animalier, Aventure, Humour</span></li>
    if let Ok(sel) = Selector::parse("span.style-serie") {
        if let Some(el) = doc.select(&sel).next() {
            let text = el.text().collect::<String>();
            details.genres = text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    // Parution: <li><label>Parution :</label><span class="parution-serie">Série finie</span></li>
    if let Ok(sel) = Selector::parse("span.parution-serie") {
        if let Some(el) = doc.select(&sel).next() {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                details.status = Some(text);
            }
        }
    }

    // Origine and Langue from page text (no dedicated CSS class)
    let page_text = doc.root_element().text().collect::<String>();

    if let Some(val) = extract_info_value(&page_text, "Origine") {
        let val = val.lines().next().unwrap_or(val).trim();
        if !val.is_empty() {
            details.origin = Some(val.to_string());
        }
    }

    if let Some(val) = extract_info_value(&page_text, "Langue") {
        let val = val.lines().next().unwrap_or(val).trim();
        if !val.is_empty() {
            details.language = Some(val.to_string());
        }
    }

    // Album count from serie-info text (e.g. "Tomes : 8")
    if let Ok(re) = regex::Regex::new(r"Tomes?\s*:\s*(\d+)") {
        if let Some(caps) = re.captures(&page_text) {
            if let Ok(n) = caps[1].parse::<i32>() {
                details.album_count = Some(n);
            }
        }
    }

    // Start year from first <meta itemprop="datePublished" content="YYYY-MM-DD">
    if let Ok(sel) = Selector::parse(r#"[itemprop="datePublished"]"#) {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                // content is "YYYY-MM-DD"
                if let Some(year_str) = content.split('-').next() {
                    if let Ok(year) = year_str.parse::<i32>() {
                        details.start_year = Some(year);
                    }
                }
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

    // Each album block starts before a .album-main div.
    // The cover image (<img itemprop="image">) is OUTSIDE .album-main (sibling),
    // so we iterate over a broader parent. But the simplest approach: parse all
    // itemprop elements relative to each .album-main, plus pick covers separately.
    let album_sel = Selector::parse(".album-main").map_err(|e| format!("selector: {e}"))?;

    // Pre-collect cover images — they appear in <img itemprop="image"> before each .album-main
    // and link to an album URL containing the book ID
    let cover_sel = Selector::parse(r#"img[itemprop="image"]"#).map_err(|e| format!("selector: {e}"))?;
    let covers: Vec<String> = doc.select(&cover_sel)
        .filter_map(|el| el.value().attr("src").map(|s| {
            if s.starts_with("http") { s.to_string() } else { format!("https://www.bedetheque.com{}", s) }
        }))
        .collect();

    for (idx, album_el) in doc.select(&album_sel).enumerate() {
        // Title from <a class="titre" title="..."> — the title attribute is clean
        let title_sel = Selector::parse("a.titre").ok();
        let title_el = title_sel.as_ref().and_then(|s| album_el.select(s).next());
        let title = title_el
            .and_then(|el| el.value().attr("title"))
            .unwrap_or("")
            .trim()
            .to_string();

        if title.is_empty() {
            continue;
        }

        // External book ID from album URL (e.g. "...-1063.html")
        let album_url = title_el.and_then(|el| el.value().attr("href")).unwrap_or("");
        let external_book_id = regex::Regex::new(r"-(\d+)\.html")
            .ok()
            .and_then(|re| re.captures(album_url))
            .map(|c| c[1].to_string())
            .unwrap_or_default();

        // Volume number from URL pattern "Tome-{N}-" or from itemprop name
        let volume_number = regex::Regex::new(r"(?i)Tome-(\d+)-")
            .ok()
            .and_then(|re| re.captures(album_url))
            .and_then(|c| c[1].parse::<i32>().ok())
            .or_else(|| extract_volume_from_title(&title));

        // Authors from itemprop="author" and itemprop="illustrator"
        let mut authors = Vec::new();
        let author_sel = Selector::parse(r#"[itemprop="author"]"#).ok();
        let illustrator_sel = Selector::parse(r#"[itemprop="illustrator"]"#).ok();
        for sel in [&author_sel, &illustrator_sel].into_iter().flatten() {
            for el in album_el.select(sel) {
                let name = el.text().collect::<String>().trim().to_string();
                // Names are "Last, First" format — normalize to "First Last"
                let normalized = if let Some((last, first)) = name.split_once(',') {
                    format!("{} {}", first.trim(), last.trim())
                } else {
                    name
                };
                if !normalized.is_empty() && is_real_author(&normalized) && !authors.contains(&normalized) {
                    authors.push(normalized);
                }
            }
        }

        // ISBN from <span itemprop="isbn">
        let isbn = Selector::parse(r#"[itemprop="isbn"]"#)
            .ok()
            .and_then(|s| album_el.select(&s).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // Page count from <span itemprop="numberOfPages">
        let page_count = Selector::parse(r#"[itemprop="numberOfPages"]"#)
            .ok()
            .and_then(|s| album_el.select(&s).next())
            .and_then(|el| el.text().collect::<String>().trim().parse::<i32>().ok());

        // Publish date from <meta itemprop="datePublished" content="YYYY-MM-DD">
        let publish_date = Selector::parse(r#"[itemprop="datePublished"]"#)
            .ok()
            .and_then(|s| album_el.select(&s).next())
            .and_then(|el| el.value().attr("content").map(|c| c.trim().to_string()))
            .filter(|s| !s.is_empty());

        // Cover from pre-collected covers (same index)
        let cover_url = covers.get(idx).cloned();

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

/// Filter out placeholder author names from Bédéthèque
fn is_real_author(name: &str) -> bool {
    !name.starts_with('<') && !name.ends_with('>') && name != "Collectif"
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

/// Normalize a title by removing French articles (leading or in parentheses)
/// and extra whitespace/punctuation, so that "Les Légendaires - Résistance"
/// and "Légendaires (Les) - Résistance" produce the same canonical form.
fn normalize_title(s: &str) -> String {
    let lower = s.to_lowercase();
    // Remove articles in parentheses: "(les)", "(la)", "(le)", "(l')", "(un)", "(une)", "(des)"
    let re_parens = regex::Regex::new(r"\s*\((?:les?|la|l'|une?|des|du|d')\)").unwrap();
    let cleaned = re_parens.replace_all(&lower, "");
    // Remove leading articles: "les ", "la ", "le ", "l'", "un ", "une ", "des ", "du ", "d'"
    let re_leading = regex::Regex::new(r"^(?:les?|la|l'|une?|des|du|d')\s+").unwrap();
    let cleaned = re_leading.replace(&cleaned, "");
    // Collapse multiple spaces/dashes into single
    let re_spaces = regex::Regex::new(r"\s+").unwrap();
    re_spaces.replace_all(cleaned.trim(), " ").to_string()
}

fn compute_confidence(title: &str, query: &str) -> f32 {
    let title_lower = title.to_lowercase();
    let query_lower = query.to_lowercase();
    if title_lower == query_lower {
        return 1.0;
    }

    // Try with normalized forms (handles Bedetheque's "Name (Article)" convention)
    let title_norm = normalize_title(title);
    let query_norm = normalize_title(query);
    if title_norm == query_norm {
        return 1.0;
    }

    if title_lower.starts_with(&query_lower) || query_lower.starts_with(&title_lower) {
        0.85
    } else if title_norm.starts_with(&query_norm) || query_norm.starts_with(&title_norm) {
        0.85
    } else if title_lower.contains(&query_lower) || query_lower.contains(&title_lower) {
        0.7
    } else if title_norm.contains(&query_norm) || query_norm.contains(&title_norm) {
        0.7
    } else {
        let common: usize = query_lower
            .chars()
            .filter(|c| title_lower.contains(*c))
            .count();
        let max_len = query_lower.len().max(title_lower.len()).max(1);
        (common as f32 / max_len as f32).clamp(0.1, 0.6)
    }
}
