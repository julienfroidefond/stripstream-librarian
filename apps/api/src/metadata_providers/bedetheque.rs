use scraper::{Html, Selector};

use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const BEDETHEQUE_BASE_URL: &str = "https://www.bedetheque.com";

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
        Box::pin(async move { search_series_impl(&query, &config, BEDETHEQUE_BASE_URL).await })
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
        Box::pin(async move { get_series_books_impl(&external_id, &config, BEDETHEQUE_BASE_URL).await })
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
    base_url: &str,
) -> Result<Vec<SeriesCandidate>, String> {
    let client = build_client()?;

    // Use the full-text search page
    let url = format!(
        "{}/search/tout?RechTexte={}&RechWhere=0",
        base_url,
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
                "{}/cache/thb_series/PlancheS_{}.jpg",
                base_url, series_id
            );

            let absolute_href = if href.starts_with("http") {
                href.clone()
            } else {
                format!("{}{}", base_url, href)
            };

            candidates.push(SeriesCandidate {
                external_id: series_id.clone(),
                title: title.clone(),
                authors: vec![],
                description: None,
                publishers: vec![],
                start_year: None,
                total_volumes: None,
                cover_url: Some(cover_url),
                external_url: Some(absolute_href),
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
            if let Ok(details) = fetch_series_details(&client, &c.external_id, c.external_url.as_deref(), base_url).await {
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
                if let Some(cover) = details.cover_url {
                    c.cover_url = Some(cover);
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
    cover_url: Option<String>,
}

async fn fetch_series_details(
    client: &reqwest::Client,
    series_id: &str,
    series_url: Option<&str>,
    base_url: &str,
) -> Result<SeriesDetails, String> {
    // Build URL — append __10000 to get all albums on one page
    let url = match series_url {
        Some(u) => {
            // Replace .html with __10000.html
            u.replace(".html", "__10000.html")
        }
        None => format!(
            "{}/serie-{}-BD-Serie__10000.html",
            base_url, series_id
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
        cover_url: None,
    };

    // Cover: first itemprop="image" (album cover from the series listing, not sidebar/recommendations)
    if let Ok(sel) = Selector::parse(r#"img[itemprop="image"]"#) {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(src) = el.value().attr("src") {
                // Replace thumbnail with full-size cover
                details.cover_url = Some(src.replace("/cache/thb_couv/", "/media/Couvertures/").to_string());
            }
        }
    }

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
    base_url: &str,
) -> Result<Vec<BookCandidate>, String> {
    let client = build_client()?;

    // We need to find the series URL — try a direct fetch
    // external_id is the numeric series ID
    // We try to fetch the series page to get the album list
    let url = format!(
        "{}/serie-{}-BD-Serie__10000.html",
        base_url, external_id
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
            "{}/serie-{}__10000.html",
            base_url, external_id
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
            if s.starts_with("http") { s.to_string() } else { format!("{}{}", base_url, s) }
        }))
        .collect();

    static RE_TOME: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)-Tome-\d+-").unwrap());
    static RE_BOOK_ID: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"-(\d+)\.html").unwrap());
    static RE_VOLUME: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)Tome-(\d+)-").unwrap());

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

        // Only keep main tomes — their URLs contain "Tome-{N}-"
        // Skip hors-série (HS), intégrales (INT/INTFL), romans, coffrets, etc.
        if !RE_TOME.is_match(album_url) {
            continue;
        }

        let external_book_id = RE_BOOK_ID
            .captures(album_url)
            .map(|c| c[1].to_string())
            .unwrap_or_default();

        // Volume number from URL pattern "Tome-{N}-" or from itemprop name
        let volume_number = RE_VOLUME
            .captures(album_url)
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

    if title_lower.starts_with(&query_lower) || query_lower.starts_with(&title_lower)
        || title_norm.starts_with(&query_norm) || query_norm.starts_with(&title_norm)
    {
        0.85
    } else if title_lower.contains(&query_lower) || query_lower.contains(&title_lower)
        || title_norm.contains(&query_norm) || query_norm.contains(&title_norm)
    {
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

// ─── Discovery: Indispensables ──────────────────────────────────────────────

/// Fetch the "Indispensables" (top voted series) from Bédéthèque.
/// Optionally filter by genre slug (e.g., "Manga", "Policier", "Science-fiction").
pub async fn fetch_indispensables(
    genre: Option<&str>,
    limit: usize,
) -> Result<Vec<SeriesCandidate>, String> {
    fetch_indispensables_with_base_url(genre, limit, BEDETHEQUE_BASE_URL).await
}

async fn fetch_indispensables_with_base_url(
    genre: Option<&str>,
    limit: usize,
    base_url: &str,
) -> Result<Vec<SeriesCandidate>, String> {
    let client = build_client()?;

    let url = match genre {
        Some(g) => format!(
            "{}/indispensables-style-{}.html",
            base_url, urlencoded(g)
        ),
        None => format!("{}/indispensables.html", base_url),
    };

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch indispensables: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Indispensables page returned {}", resp.status()));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read indispensables page: {e}"))?;

    // Check for rate limiting
    if html.contains("<title></title>") {
        return Err("Bedetheque: IP may be rate-limited, please retry later".to_string());
    }

    // Parse in a block to drop non-Send types (Html, Selector) before async enrichment
    let mut candidates = Vec::new();
    {
    let document = Html::parse_document(&html);

    let serie_sel = Selector::parse("span.serie a").unwrap();
    let _numero_sel = Selector::parse("span.numero").unwrap();
    let style_sel = Selector::parse("span.style").unwrap();

    // Also get cover images from the gallery
    let gallery_sel = Selector::parse("ul.gallery-couv li a").unwrap();
    let img_sel = Selector::parse("img").unwrap();

    // Collect cover URLs indexed by series URL
    let mut cover_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for a_el in document.select(&gallery_sel) {
        if let Some(href) = a_el.value().attr("href") {
            if let Some(img) = a_el.select(&img_sel).next() {
                if let Some(src) = img.value().attr("src") {
                    cover_map.insert(href.to_string(), src.to_string());
                }
            }
        }
    }

    // Parse the ranked list
    // The ranking entries are inside the main content, structured as siblings
    // We need to find all span.serie > a elements and their surrounding context
    let mut seen_ids = std::collections::HashSet::new();

    // Walk through all serie links in the page
    for serie_a in document.select(&serie_sel) {
        let title = serie_a.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let href = match serie_a.value().attr("href") {
            Some(h) => h.to_string(),
            None => continue,
        };

        // Extract series ID from URL
        let series_id = SERIES_URL_RE
            .captures(&href)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

        let Some(sid) = &series_id else { continue };
        if !seen_ids.insert(sid.clone()) {
            continue; // deduplicate
        }

        // Get cover from gallery, fallback to series thumbnail URL
        let cover_url = cover_map.get(&href).cloned().or_else(|| {
            Some(format!(
                "{}/cache/thb_series/PlancheS_{}.jpg",
                base_url, sid
            ))
        });

        // Try to get genre from the sibling span.style
        // Navigate to parent and find span.style
        let genre_text = serie_a
            .parent()
            .and_then(|p| p.parent())
            .and_then(|grandparent| {
                scraper::ElementRef::wrap(grandparent)
                    .and_then(|el| el.select(&style_sel).next())
                    .map(|s| s.text().collect::<String>().trim().to_string())
            })
            .filter(|g| !g.is_empty());

        let genres = genre_text.map(|g| vec![g]).unwrap_or_default();

        // Confidence based on rank position (first = 1.0, decreasing)
        let rank = candidates.len();
        let confidence = (1.0 - (rank as f32 / 100.0)).clamp(0.1, 1.0);

        let absolute_href = if href.starts_with("http") {
            href
        } else {
            format!("{}{}", base_url, href)
        };

        candidates.push(SeriesCandidate {
            external_id: sid.clone(),
            title,
            authors: vec![],
            description: None,
            publishers: vec![],
            start_year: None,
            total_volumes: None,
            cover_url,
            external_url: Some(absolute_href),
            confidence,
            metadata_json: serde_json::json!({
                "genres": genres,
                "source": "indispensables",
            }),
        });

        if candidates.len() >= limit {
            break;
        }
    }
    } // drop document, selectors (non-Send) before async enrichment

    // Enrich covers for candidates that only have the PlancheS_ fallback (top 20)
    for c in candidates.iter_mut() {
        if c.cover_url.as_ref().is_some_and(|u| u.contains("/Couvertures/") || u.contains("/thb_couv/")) {
            continue;
        }
        let Some(ref url) = c.external_url else { continue };
        let page_url = url.replace(".html", "__10000.html");
        let cover = fetch_series_cover(&client, &page_url).await;
        if let Some(cover) = cover {
            c.cover_url = Some(cover);
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    Ok(candidates)
}

/// Fetch the real album cover URL from a series page (itemprop="image" of the first tome).
async fn fetch_series_cover(client: &reqwest::Client, page_url: &str) -> Option<String> {
    let resp = client.get(page_url).send().await.ok()?;
    let html = resp.text().await.ok()?;
    if html.contains("<title></title>") { return None; }
    // Parse in a block to drop non-Send types before any .await
    let cover = {
        let doc = Html::parse_document(&html);
        let sel = Selector::parse(r#"img[itemprop="image"]"#).ok()?;
        doc.select(&sel).next()
            .and_then(|el| el.value().attr("src"))
            .map(|src| src.replace("/cache/thb_couv/", "/media/Couvertures/").to_string())
    };
    cover
}

static SERIES_URL_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"/serie-(\d+)-").unwrap());

#[cfg(test)]
mod tests {
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

        let results = search_series_impl("Blacksad", &config(), &server.uri()).await.unwrap();

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
        assert!(results[0].description.as_ref().unwrap().contains("policière animalière"));
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

        let results = search_series_impl("TestSerie", &config(), &server.uri()).await.unwrap();
        assert_eq!(results.len(), 1);

        // Cover URL should use the mock server base, not hardcoded bedetheque.com
        let cover = results[0].cover_url.as_ref().unwrap();
        assert!(
            cover.starts_with(&server.uri()),
            "cover URL should use mock base URL, got: {cover}"
        );
        assert!(cover.contains("/cache/thb_series/PlancheS_99.jpg"));
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

        let books = get_series_books_impl("3", &config(), &server.uri()).await.unwrap();

        assert_eq!(books.len(), 2);

        assert_eq!(books[0].title, "Quelque part entre les ombres");
        assert_eq!(books[0].volume_number, Some(1));
        assert_eq!(books[0].external_book_id, "1063");
        assert_eq!(books[0].isbn.as_deref(), Some("978-2-87129-410-1"));
        assert_eq!(books[0].page_count, Some(48));
        assert_eq!(books[0].publish_date.as_deref(), Some("2000-11-01"));

        // Cover URL from pre-collected covers
        let cover = books[0].cover_url.as_ref().unwrap();
        assert!(cover.contains("Couv_100"), "first book should get first cover, got: {cover}");

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

        let books = get_series_books_impl("1", &config(), &server.uri()).await.unwrap();
        // Only the Tome-1 album should be kept, the INT one should be filtered out
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "Tome 1");
    }

    // ── fetch_indispensables ────────────────────────────────────────────

    #[tokio::test]
    async fn fetch_indispensables_parses_series_list() {
        let server = MockServer::start().await;

        let html = r#"
        <html>
        <head><title>Indispensables</title></head>
        <body>
            <ul class="gallery-couv">
                <li><a href="/serie-3-BD-Blacksad.html"><img src="/cache/thb_couv/Couv_3.jpg"></a></li>
                <li><a href="/serie-42-BD-Asterix.html"><img src="/cache/thb_couv/Couv_42.jpg"></a></li>
            </ul>
            <div>
                <span class="serie"><a href="/serie-3-BD-Blacksad.html">Blacksad</a></span>
                <span class="style">Policier</span>
            </div>
            <div>
                <span class="serie"><a href="/serie-42-BD-Asterix.html">Astérix</a></span>
                <span class="style">Humour</span>
            </div>
        </body>
        </html>"#;

        Mock::given(method("GET"))
            .and(path_regex(r"/indispensables.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(html))
            .mount(&server)
            .await;

        // Mock the cover enrichment pages — return 404 so it falls back gracefully
        Mock::given(method("GET"))
            .and(path_regex(r"/serie-\d+-.*__10000\.html"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let results = fetch_indispensables_with_base_url(None, 10, &server.uri()).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Blacksad");
        assert_eq!(results[0].external_id, "3");
        assert_eq!(results[1].title, "Astérix");
        assert_eq!(results[1].external_id, "42");

        // Cover URLs should come from the gallery
        let cover0 = results[0].cover_url.as_ref().unwrap();
        assert!(cover0.contains("Couv_3"), "should use gallery cover, got: {cover0}");
    }

    #[tokio::test]
    async fn fetch_indispensables_rate_limited() {
        let server = MockServer::start().await;

        let html = "<html><head><title></title></head><body></body></html>";

        Mock::given(method("GET"))
            .and(path_regex(r"/indispensables.*"))
            .respond_with(ResponseTemplate::new(200).set_body_string(html))
            .mount(&server)
            .await;

        let result = fetch_indispensables_with_base_url(None, 10, &server.uri()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rate-limited"));
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
        assert_eq!(compute_confidence("Les Aventures de Blacksad", "blacksad"), 0.7);
    }

    #[test]
    fn compute_confidence_normalized_match() {
        // "Légendaires (Les)" normalized == "légendaires" == "Les Légendaires" normalized
        assert_eq!(compute_confidence("Légendaires (Les)", "Les Légendaires"), 1.0);
    }

    #[test]
    fn extract_volume_from_title_works() {
        assert_eq!(extract_volume_from_title("Tome 3 - Blah"), Some(3));
        assert_eq!(extract_volume_from_title("Vol. 12"), Some(12));
        assert_eq!(extract_volume_from_title("#5 something"), Some(5));
        assert_eq!(extract_volume_from_title("No volume here"), None);
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
}
