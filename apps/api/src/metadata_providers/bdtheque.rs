use scraper::{Html, Selector};

use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const BASE_URL: &str = "https://www.bdtheque.com";

pub struct BdthequeProvider;

impl MetadataProvider for BdthequeProvider {
    fn name(&self) -> &str {
        "bdtheque"
    }

    fn search_series(
        &self,
        query: &str,
        _config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<SeriesCandidate>, String>> + Send + '_>,
    > {
        let query = query.to_string();
        Box::pin(async move { search_series_impl(&query, BASE_URL).await })
    }

    fn get_series(
        &self,
        external_id: &str,
        _config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<SeriesCandidate, String>> + Send + '_>,
    > {
        let external_id = external_id.to_string();
        Box::pin(async move { get_series_impl(&external_id, BASE_URL).await })
    }

    fn get_series_books(
        &self,
        external_id: &str,
        _config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<BookCandidate>, String>> + Send + '_>,
    > {
        let external_id = external_id.to_string();
        Box::pin(async move { get_series_books_impl(&external_id, BASE_URL).await })
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("StripstreamLibrarian/1.0 (metadata; contact administrator)")
        .build()
        .map_err(|e| format!("failed to build BDTheque client: {e}"))
}

fn text(el: scraper::ElementRef<'_>) -> String {
    el.text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn absolute(base: &str, href: &str) -> String {
    reqwest::Url::parse(base)
        .ok()
        .and_then(|base| base.join(href).ok())
        .map(|url| url.to_string())
        .unwrap_or_else(|| href.to_string())
}

/// The default search layout shows covers only: the anchor wraps an image and
/// has no text. Fall back to the image alt / anchor title in that case.
fn result_title(a: scraper::ElementRef<'_>) -> String {
    let direct = text(a);
    if !direct.is_empty() {
        return direct;
    }
    if let Some(alt) = a
        .select(&Selector::parse("img").unwrap())
        .next()
        .and_then(|img| img.value().attr("alt"))
    {
        let cleaned = alt
            .trim_start_matches("Couverture de la série ")
            .trim_start_matches("Couverture de l'album ")
            .trim();
        if !cleaned.is_empty() {
            return cleaned.to_string();
        }
    }
    if let Some(title) = a.value().attr("title") {
        let cleaned = title
            .trim_start_matches("Accéder à la série BD ")
            .trim_start_matches("Accéder à la série ")
            .trim();
        if !cleaned.is_empty() {
            return cleaned.to_string();
        }
    }
    String::new()
}

/// Cover view lazy-loads the real image through `data-echo`.
fn result_cover(a: scraper::ElementRef<'_>) -> Option<String> {
    let img = a.select(&Selector::parse("img").unwrap()).next()?;
    let src = img
        .value()
        .attr("data-echo")
        .or_else(|| img.value().attr("data-src"))
        .or_else(|| img.value().attr("src"))?;
    if src.contains("placeholder") {
        return None;
    }
    Some(src.to_string())
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

fn series_external_id(href: &str) -> Option<String> {
    let url = reqwest::Url::parse(href).ok()?;
    let path = url.path().trim_matches('/');
    path.strip_prefix("series/").map(str::to_string)
}

async fn get_html(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("BDTheque request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("BDTheque returned {}", response.status()));
    }
    response
        .text()
        .await
        .map_err(|e| format!("failed to read BDTheque response: {e}"))
}

async fn search_series_impl(query: &str, base: &str) -> Result<Vec<SeriesCandidate>, String> {
    let c = client()?;
    let url = format!("{base}/recherche/series/");
    let html = c
        .post(&url)
        .form(&[("search", query)])
        .send()
        .await
        .map_err(|e| format!("BDTheque search failed: {e}"))?;
    if !html.status().is_success() {
        return Err(format!("BDTheque returned {}", html.status()));
    }
    let body = html
        .text()
        .await
        .map_err(|e| format!("failed to read BDTheque search: {e}"))?;
    let mut results = {
        let doc = Html::parse_document(&body);
        let selector = Selector::parse("a[href*='/series/']").map_err(|e| e.to_string())?;
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        for a in doc.select(&selector) {
            let href = match a.value().attr("href") {
                Some(v) => v,
                None => continue,
            };
            let href_absolute = absolute(base, href);
            let Some(id) = series_external_id(&href_absolute) else {
                continue;
            };
            let title = result_title(a);
            if title.is_empty() || !seen.insert(id.clone()) {
                continue;
            }
            let confidence = if title.eq_ignore_ascii_case(query) {
                1.0
            } else if title.to_lowercase().contains(&query.to_lowercase()) {
                0.7
            } else {
                0.3
            };
            let cover_url = result_cover(a).map(|v| absolute(base, &v));
            let metadata_json = match &cover_url {
                Some(cover) => serde_json::json!({"provider": "bdtheque", "cover_url": cover}),
                None => serde_json::json!({"provider": "bdtheque"}),
            };
            results.push(SeriesCandidate {
                external_id: id.clone(),
                title,
                authors: vec![],
                description: None,
                publishers: vec![],
                start_year: None,
                total_volumes: None,
                cover_url,
                external_url: Some(absolute(base, href)),
                confidence,
                metadata_json,
            });
        }
        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(20);
        results
    }; // document dropped before the enrichment awaits below
    enrich_series_details(base, &mut results).await;
    Ok(results)
}

/// The search layout only exposes a title and a cover. Fetch the series page
/// for the best candidates so that authors, publisher, year, status and volume
/// count are available when the series is added.
async fn enrich_series_details(base: &str, candidates: &mut [SeriesCandidate]) {
    const MAX_ENRICHED: usize = 8;
    let fetches = candidates.iter().take(MAX_ENRICHED).map(|candidate| {
        let external_id = candidate.external_id.clone();
        let base = base.to_string();
        async move { get_series_impl(&external_id, &base).await.ok() }
    });
    let details = futures::future::join_all(fetches).await;
    for (candidate, detail) in candidates.iter_mut().take(MAX_ENRICHED).zip(details) {
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
    let c = client()?;
    let path = external_id.trim_start_matches('/');
    let url = format!("{base}/series/{path}");
    let body = get_html(&c, &url).await?;
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
    let c = client()?;
    let id = series_id(external_id).ok_or("invalid BDTheque series ID")?;
    let mut books = Vec::new();
    let mut page = 0u32;
    loop {
        let url = format!("{base}/ajax/series/tomes/{id}/{page}");
        let body = get_html(&c, &url).await?;
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
