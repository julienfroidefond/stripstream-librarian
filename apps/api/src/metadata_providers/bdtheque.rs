use scraper::{Html, Selector};
use serde::Deserialize;

use super::scraping::{self, absolute, text};
use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const BASE_URL: &str = "https://www.bdtheque.com";

/// Directory where BDTheque stores series covers.
const COVER_DIR: &str = "/repupload/T/";

pub struct BdthequeProvider;

#[async_trait::async_trait]
impl MetadataProvider for BdthequeProvider {
    fn name(&self) -> &str {
        "bdtheque"
    }

    async fn search_series(
        &self,
        query: &str,
        _config: &ProviderConfig,
    ) -> Result<Vec<SeriesCandidate>, String> {
        search_series_impl(query, BASE_URL).await
    }

    async fn get_series(
        &self,
        external_id: &str,
        _config: &ProviderConfig,
    ) -> Result<SeriesCandidate, String> {
        get_series_impl(external_id, BASE_URL).await
    }

    async fn get_series_books(
        &self,
        external_id: &str,
        _config: &ProviderConfig,
    ) -> Result<Vec<BookCandidate>, String> {
        get_series_books_impl(external_id, BASE_URL).await
    }
}

/// One entry of the site's typeahead response.
#[derive(Debug, Deserialize)]
struct SearchHit {
    id: String,
    #[serde(default)]
    nom: String,
    #[serde(default)]
    couv: Option<String>,
}

/// Percent-encode `value` as a single URL path segment.
///
/// `reqwest::Url`'s path-segment encoder keeps RFC 3986 sub-delimiters (`'`,
/// `!`, `(`, `)`, `&`, `+`, ...) literal, but BDTheque's Express router answers
/// `400 Bad Request` for them. Encode everything outside the unreserved set,
/// which the site accepts (`%27`, `%21`, `%28`, `%29`, `%26`, `%2B`).
fn encode_path_segment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0F) as usize] as char);
        }
    }
    encoded
}

/// Build the typeahead URL. The query is a path segment (not a query string),
/// and must be fully percent-encoded (see [`encode_path_segment`]).
fn search_url(base: &str, query: &str) -> Result<String, String> {
    let base = reqwest::Url::parse(base).map_err(|e| format!("invalid BDTheque base URL: {e}"))?;
    let query = query.replace('/', " ");
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(format!(
        "{}/ajax/search/series/{}",
        base.as_str().trim_end_matches('/'),
        encode_path_segment(&query)
    ))
}

/// Parse the typeahead payload. The site answers `null` (not `[]`) when nothing matches.
fn parse_search_hits(body: &str) -> Result<Vec<SearchHit>, String> {
    let hits = serde_json::from_str::<Option<Vec<SearchHit>>>(body)
        .map_err(|e| format!("invalid BDTheque search response: {e}"))?;
    Ok(hits.unwrap_or_default())
}

fn candidate_from_hit(
    hit: SearchHit,
    base: &str,
    query: &str,
    max_confidence: f32,
) -> Option<SeriesCandidate> {
    let id = hit.id.trim();
    let title = hit.nom.trim();
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let cover_url = hit
        .couv
        .as_deref()
        .map(str::trim)
        .filter(|couv| !couv.is_empty())
        .map(|couv| absolute(base, &format!("{COVER_DIR}{couv}")));
    let external_url = absolute(base, &format!("/series/{id}"));
    let metadata_json = match &cover_url {
        Some(cover) => serde_json::json!({"provider": "bdtheque", "cover_url": cover}),
        None => serde_json::json!({"provider": "bdtheque"}),
    };
    Some(SeriesCandidate {
        external_id: id.to_string(),
        title: title.to_string(),
        authors: vec![],
        description: None,
        publishers: vec![],
        start_year: None,
        total_volumes: None,
        cover_url,
        external_url: Some(external_url),
        confidence: scraping::confidence_for(title, query, max_confidence),
        metadata_json,
    })
}

/// BDTheque lists authors as "Nom (Prénom)"; normalise to "Prénom Nom".
fn normalize_author_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(open) = trimmed.rfind('(') {
        if trimmed.ends_with(')') {
            let last = trimmed[..open].trim();
            let first = trimmed[open + 1..trimmed.len() - 1].trim();
            if !last.is_empty() && !first.is_empty() {
                return format!("{first} {last}");
            }
        }
    }
    trimmed.to_string()
}

fn series_id(path: &str) -> Option<String> {
    let mut parts = path.trim_matches('/').split('/');
    let first = parts.next()?;
    if first == "series" {
        parts.next().map(str::to_string)
    } else {
        first.parse::<u64>().ok().map(|_| first.to_string())
    }
}

/// Parse a typeahead payload into scored, deduplicated candidates.
fn candidates_from_body(
    body: &str,
    base: &str,
    query: &str,
    max_confidence: f32,
) -> Result<Vec<SeriesCandidate>, String> {
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();
    for hit in parse_search_hits(body)? {
        let Some(candidate) = candidate_from_hit(hit, base, query, max_confidence) else {
            continue;
        };
        if seen.insert(candidate.external_id.clone()) {
            results.push(candidate);
        }
    }
    results.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(20);
    Ok(results)
}

async fn search_series_impl(query: &str, base: &str) -> Result<Vec<SeriesCandidate>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let c = scraping::shared_client(20, "BDTheque")?;
    // `/recherche/series/` is behind a Cloudflare managed challenge (403 "Just a
    // moment..."): use the JSON typeahead endpoint the site itself calls instead.
    //
    // The endpoint only answers when every query word exists in a title, so a
    // local name such as `Cités obscures (Les)` returns nothing. Retry with the
    // normalised variants (article moved to the front, then without the
    // subtitle) and keep the first non-empty answer.
    let mut results = Vec::new();
    for variant in scraping::query_variants(query) {
        let url = search_url(base, &variant.query)?;
        let body = scraping::get_html(&c, &url, "BDTheque").await?;
        results = candidates_from_body(&body, base, &variant.query, variant.max_confidence)?;
        if !results.is_empty() {
            break;
        }
    }

    enrich_series_details(base, &mut results).await;
    Ok(results)
}

/// The typeahead only exposes a title and a cover. Fetch the series page for the
/// best candidates so that authors, publisher, year, status and volume count are
/// available when the series is added.
async fn enrich_series_details(base: &str, candidates: &mut [SeriesCandidate]) {
    let details = scraping::fetch_candidate_details(base, candidates, |external_id, base| {
        Box::pin(async move { get_series_impl(&external_id, &base).await.ok() })
    })
    .await;
    for (candidate, detail) in candidates.iter_mut().zip(details) {
        let Some(detail) = detail else { continue };
        candidate.authors = detail.authors;
        candidate.description = detail.description;
        candidate.publishers = detail.publishers;
        candidate.start_year = detail.start_year;
        candidate.total_volumes = detail.total_volumes;
        candidate.external_url = detail.external_url;
        candidate.metadata_json = detail.metadata_json;
        if detail.cover_url.is_some() {
            candidate.cover_url = detail.cover_url;
        } else if let Some(cover) = candidate.cover_url.clone() {
            if let Some(obj) = candidate.metadata_json.as_object_mut() {
                obj.insert("cover_url".to_string(), serde_json::json!(cover));
            }
        }
    }
}

/// Read a number that may be encoded as a JSON number or a string.
fn json_number(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|s| s.replace(',', ".").parse().ok())
    })
}

/// BDTheque exposes the series community rating as a schema.org
/// `AggregateRating` inside a JSON-LD block. Returns `(rating, count, scale)`.
fn parse_aggregate_rating(doc: &Html) -> Option<(f64, i64, f64)> {
    let sel = Selector::parse("script[type='application/ld+json']").ok()?;
    for script in doc.select(&sel) {
        let raw = script.text().collect::<Vec<_>>().join(" ");
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let Some(agg) = json.get("aggregateRating") else {
            continue;
        };
        let Some(rating) = agg.get("ratingValue").and_then(json_number) else {
            continue;
        };
        let count = agg
            .get("ratingCount")
            .and_then(json_number)
            .map(|c| c.round() as i64)
            .unwrap_or(0);
        let scale = agg
            .get("bestRating")
            .and_then(json_number)
            .filter(|s| *s > 0.0)
            .unwrap_or(5.0);
        if rating > 0.0 && count > 0 {
            return Some((rating, count, scale));
        }
    }
    None
}

async fn get_series_impl(external_id: &str, base: &str) -> Result<SeriesCandidate, String> {
    let c = scraping::shared_client(20, "BDTheque")?;
    let path = external_id.trim_start_matches('/');
    let url = format!("{base}/series/{path}");
    let body = scraping::get_html(&c, &url, "BDTheque").await?;
    let doc = Html::parse_document(&body);
    let h1 = Selector::parse("h1").unwrap();
    let title = doc
        .select(&h1)
        .next()
        .map(text)
        .filter(|s| !s.is_empty())
        .ok_or("BDTheque series title missing")?;
    let description = Selector::parse("meta[name='Description']")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .and_then(|e| e.value().attr("content"))
        .map(str::to_string);
    let cover_url = Selector::parse("meta[property='og:image']")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .and_then(|e| e.value().attr("content"))
        .map(str::to_string);
    let mut authors = Vec::new();
    let mut publishers = Vec::new();
    let mut start_year = None;
    let mut total_volumes = None;
    let mut status = None;
    let row_sel = Selector::parse("table tr").unwrap();
    for row in doc.select(&row_sel) {
        let cells: Vec<String> = row
            .select(&Selector::parse("td").unwrap())
            .map(text)
            .collect();
        if cells.len() < 2 {
            continue;
        }
        let label = cells[0].to_lowercase();
        if label.contains("scénario") || label.contains("dessin") {
            authors.extend(
                row.select(&Selector::parse("td:nth-child(2) a").unwrap())
                    .map(text)
                    .map(|name| normalize_author_name(&name)),
            );
        }
        if label.contains("editeur") {
            if let Some(a) = row
                .select(&Selector::parse("td:nth-child(2) a").unwrap())
                .next()
            {
                publishers.push(text(a));
            }
        }
        if label.contains("date de parution") {
            start_year = cells[1].split_whitespace().find_map(|v| {
                let digits: String = v.chars().filter(char::is_ascii_digit).collect();
                (digits.len() == 4).then(|| digits.parse().ok()).flatten()
            });
        }
        if label.contains("statut") {
            status = row
                .select(&Selector::parse("td:nth-child(2) a").unwrap())
                .next()
                .map(text)
                .filter(|s| !s.is_empty())
                .or_else(|| Some(cells[1].clone()));
            total_volumes = cells[1]
                .split("tomes")
                .next()
                .and_then(|v| v.split_whitespace().last())
                .and_then(|v| v.parse().ok());
        }
    }
    authors.dedup();
    publishers.sort();
    publishers.dedup();
    let rating = parse_aggregate_rating(&doc);
    Ok(SeriesCandidate {
        external_id: path.to_string(),
        title,
        authors: authors.clone(),
        description: description.clone(),
        publishers: publishers.clone(),
        start_year,
        total_volumes,
        cover_url: cover_url.clone(),
        external_url: Some(url),
        confidence: 1.0,
        metadata_json: serde_json::json!({
            "description": description,
            "authors": authors,
            "publishers": publishers,
            "start_year": start_year,
            "total_volumes": total_volumes,
            "status": status,
            "cover_url": cover_url,
            "rating": rating.map(|(r, _, _)| r),
            "rating_count": rating.map(|(_, c, _)| c),
            "rating_scale": rating.map(|(_, _, s)| s),
        }),
    })
}

async fn get_series_books_impl(
    external_id: &str,
    base: &str,
) -> Result<Vec<BookCandidate>, String> {
    let c = scraping::shared_client(20, "BDTheque")?;
    let id = series_id(external_id).ok_or("invalid BDTheque series ID")?;
    let mut books = Vec::new();
    let mut page = 0u32;
    loop {
        let url = format!("{base}/ajax/series/tomes/{id}/{page}");
        let body = scraping::get_html(&c, &url, "BDTheque").await?;
        let page_books = parse_tomes_page(&body, &id);
        if page_books.is_empty() {
            break;
        }
        books.extend(page_books);
        page += 1;
    }
    Ok(books)
}

fn parse_tomes_page(body: &str, id: &str) -> Vec<BookCandidate> {
    let doc = Html::parse_document(body);
    let card = Selector::parse("div.card").unwrap();
    let mut books = Vec::new();
    for item in doc.select(&card) {
        let title = item
            .select(&Selector::parse("h5").unwrap())
            .next()
            .map(text)
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let volume_number = title.split_whitespace().next().and_then(|v| v.parse().ok());
        let title = title
            .split_once('-')
            .map(|(_, t)| t.trim().to_string())
            .unwrap_or(title);
        let cover_url = item
            .select(&Selector::parse("img.cover").unwrap())
            .next()
            .and_then(|e| e.value().attr("src"))
            .map(str::to_string);
        let details = item
            .select(&Selector::parse("small").unwrap())
            .map(text)
            .collect::<Vec<_>>()
            .join(" ");
        let isbn = details
            .split("ISBN :")
            .nth(1)
            .and_then(|v| v.split_whitespace().next())
            .map(str::to_string);
        let page_count = details
            .split("Pagination :")
            .nth(1)
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok());
        let publish_date = details
            .split("Date de parution :")
            .nth(1)
            .map(|v| v.split('|').next().unwrap_or(v).trim().to_string());
        let summary = item
            .select(&Selector::parse("p.card-text").unwrap())
            .next()
            .map(text);
        let body_text = item.text().collect::<Vec<_>>().join(" ");
        let external_book_id = isbn
            .clone()
            .unwrap_or_else(|| format!("{id}-{}", volume_number.unwrap_or(0)));
        let authors = body_text
            .split("Scénario :")
            .nth(1)
            .and_then(|v| v.split("Dessin :").next())
            .map(str::trim)
            .map(|v| vec![v.to_string()])
            .unwrap_or_default();
        books.push(BookCandidate {
            external_book_id,
            title,
            volume_number,
            authors,
            isbn,
            summary,
            cover_url,
            page_count,
            language: Some("fr".into()),
            publish_date,
            metadata_json: serde_json::json!({"provider": "bdtheque"}),
        });
    }
    books
}

#[cfg(test)]
#[path = "tests/bdtheque.rs"]
mod tests;
