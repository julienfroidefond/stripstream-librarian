use scraper::{Html, Selector};

use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const BASE_URL: &str = "https://www.bdphile.fr";

pub struct BdphileProvider;

impl MetadataProvider for BdphileProvider {
    fn name(&self) -> &str {
        "bdphile"
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
        .map_err(|e| format!("failed to build BDphile client: {e}"))
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
fn title_slug(href: &str) -> Option<String> {
    let u = reqwest::Url::parse(href).ok()?;
    u.path().strip_prefix("/series/").map(str::to_string)
}

fn clean_title(value: &str) -> String {
    let trimmed = value.trim();
    match trimmed.rfind(" (") {
        Some(open) if trimmed.ends_with(')') => trimmed[..open].trim().to_string(),
        _ => trimmed.to_string(),
    }
}

#[derive(Debug, serde::Deserialize)]
struct SearchHit {
    url: String,
    text: String,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    #[serde(default)]
    series: Vec<SearchHit>,
    #[serde(default)]
    bests: Vec<SearchHit>,
}
async fn get_html(c: &reqwest::Client, url: &str) -> Result<String, String> {
    let r = c
        .get(url)
        .send()
        .await
        .map_err(|e| format!("BDphile request failed: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("BDphile returned {}", r.status()));
    }
    r.text()
        .await
        .map_err(|e| format!("failed to read BDphile response: {e}"))
}

async fn search_series_impl(query: &str, base: &str) -> Result<Vec<SeriesCandidate>, String> {
    let c = client()?;
    // BDphile's HTML search page errors out for anonymous clients. The
    // canonical search is the JSON autocomplete endpoint, which requires the
    // XHR marker to answer with JSON instead of a PHP notice.
    let mut url =
        reqwest::Url::parse(&format!("{base}/search/ajax/")).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("q", query);
    let response = c
        .get(url)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .send()
        .await
        .map_err(|e| format!("BDphile search failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("BDphile returned {}", response.status()));
    }
    let payload: SearchResponse = response
        .json()
        .await
        .map_err(|e| format!("failed to decode BDphile search: {e}"))?;

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for hit in payload.bests.into_iter().chain(payload.series) {
        let Some(id) = title_slug(&hit.url) else {
            continue;
        };
        if id.is_empty() || !seen.insert(id.clone()) {
            continue;
        }
        let title = clean_title(&hit.text);
        if title.is_empty() {
            continue;
        }
        let confidence = if title.eq_ignore_ascii_case(query) {
            1.0
        } else if title.to_lowercase().contains(&query.to_lowercase()) {
            0.7
        } else {
            0.3
        };
        out.push(SeriesCandidate {
            external_id: id,
            title,
            authors: vec![],
            description: None,
            publishers: vec![],
            start_year: None,
            total_volumes: None,
            cover_url: None,
            external_url: Some(hit.url.clone()),
            confidence,
            metadata_json: serde_json::json!({"provider": "bdphile"}),
        });
    }
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(20);
    enrich_series_details(base, &mut out).await;
    Ok(out)
}

/// BDphile's search endpoint returns no artwork or series fields, so fetch the
/// series page for the best candidates to fill them in (cover included).
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
        candidate.title = detail.title;
        candidate.authors = detail.authors;
        candidate.description = detail.description;
        candidate.publishers = detail.publishers;
        candidate.start_year = detail.start_year;
        candidate.total_volumes = detail.total_volumes;
        candidate.cover_url = detail.cover_url;
        candidate.external_url = detail.external_url;
        candidate.metadata_json = detail.metadata_json;
    }
}

/// The series cover is only present in an inline `<style>` rule for
/// `.background-series`, pointing at the representative album cover.
fn series_cover_url(doc: &Html) -> Option<String> {
    let styles = Selector::parse("style").ok()?;
    for style in doc.select(&styles) {
        let css = text(style);
        let Some(pos) = css.find("background-series") else {
            continue;
        };
        let rest = &css[pos..];
        let Some(start) = rest.find("url(\"") else {
            continue;
        };
        let after = &rest[start + 5..];
        let end = after.find('"')?;
        return Some(after[..end].to_string());
    }
    None
}

/// Series page definition lists use icon-only `<dt>` elements: the label is the
/// `title` attribute of a nested tooltip span. Returns the `<dd>` sibling.
fn dd_after_tooltip<'a>(doc: &'a Html, tooltip: &str) -> Option<scraper::ElementRef<'a>> {
    let dt_sel = Selector::parse("dt").ok()?;
    let span_sel = Selector::parse("span[title]").ok()?;
    for dt in doc.select(&dt_sel) {
        let matches = dt.select(&span_sel).any(|span| {
            span.value()
                .attr("title")
                .map(|t| t.eq_ignore_ascii_case(tooltip))
                .unwrap_or(false)
        });
        if !matches {
            continue;
        }
        let mut node = dt.next_sibling();
        while let Some(current) = node {
            if let Some(el) = scraper::ElementRef::wrap(current) {
                match el.value().name() {
                    "dd" => return Some(el),
                    "dt" => break,
                    _ => {}
                }
            }
            node = current.next_sibling();
        }
    }
    None
}

/// BDphile lists every contributor with their roles in parentheses, including
/// colorists, letterers and studios. Keep only scénario/dessin credits so the
/// series authors stay meaningful.
fn dd_authors(dd: scraper::ElementRef<'_>) -> Vec<String> {
    let div_sel = Selector::parse("div").unwrap();
    let a_sel = Selector::parse("a[href*='/author/view/']").unwrap();
    let role_re = regex::Regex::new(r"(?i)sc[ée]nario|dessin").unwrap();
    let mut all: Vec<String> = Vec::new();
    let mut filtered: Vec<String> = Vec::new();
    for div in dd.select(&div_sel) {
        let role = text(div);
        for a in div.select(&a_sel) {
            let name = text(a);
            if name.is_empty() {
                continue;
            }
            if !all.contains(&name) {
                all.push(name.clone());
            }
            if role_re.is_match(&role) && !filtered.contains(&name) {
                filtered.push(name);
            }
        }
    }
    if filtered.is_empty() {
        all
    } else {
        filtered
    }
}

fn field(doc: &Html, name: &str) -> Option<String> {
    let dts = Selector::parse("dt").ok()?;
    for dt in doc.select(&dts) {
        if !text(dt).eq_ignore_ascii_case(name) {
            continue;
        }
        let mut node = dt.next_sibling();
        while let Some(current) = node {
            if let Some(el) = scraper::ElementRef::wrap(current) {
                if el.value().name() == "dd" {
                    let value = text(el);
                    if !value.is_empty() {
                        return Some(value);
                    }
                } else {
                    break;
                }
            }
            node = current.next_sibling();
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
    let title = doc
        .select(&Selector::parse("h1").unwrap())
        .next()
        .map(text)
        .ok_or("BDphile series title missing")?;
    let description = Selector::parse("meta[name='description']")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .and_then(|e| e.value().attr("content"))
        .map(str::to_string)
        .or_else(|| {
            Selector::parse(".synopsis")
                .ok()
                .and_then(|s| doc.select(&s).next())
                .map(text)
        });
    let cover_url = series_cover_url(&doc).map(|v| absolute(base, &v));

    let authors = dd_after_tooltip(&doc, "Auteurs")
        .map(|dd| dd_authors(dd))
        .unwrap_or_default();
    let publishers = dd_after_tooltip(&doc, "Éditeurs")
        .map(|dd| {
            dd.select(&Selector::parse("a[href*='/publisher/view/']").unwrap())
                .map(text)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let publication = dd_after_tooltip(&doc, "Publication")
        .map(|dd| text(dd))
        .unwrap_or_else(|| doc.root_element().text().collect::<Vec<_>>().join(" "));

    let status = regex::Regex::new(r"(?i)(En cours|Terminé|Abandonné|Annulé|Suspendu|NC)")
        .ok()
        .and_then(|re| re.captures(&publication).map(|c| c[1].to_string()));
    let total_volumes = regex::Regex::new(r"(?i)(\d+)\s+tomes?")
        .ok()
        .and_then(|re| re.captures(&publication))
        .and_then(|c| c[1].parse().ok());
    let start_year = publication
        .split("De ")
        .nth(1)
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| {
            let digits: String = v.chars().filter(char::is_ascii_digit).collect();
            (digits.len() == 4).then(|| digits.parse().ok()).flatten()
        });

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
        }),
    })
}

async fn get_series_books_impl(
    external_id: &str,
    base: &str,
) -> Result<Vec<BookCandidate>, String> {
    let c = client()?;
    let path = external_id.trim_start_matches('/');
    let url = format!("{base}/series/{path}");
    let body = get_html(&c, &url).await?;
    let rows: Vec<(Vec<String>, String, String)> = {
        let doc = Html::parse_document(&body);
        let row_sel = Selector::parse("#detail_view tbody tr").unwrap();
        let cell_sel = Selector::parse("td").unwrap();
        let link_sel = Selector::parse("a[href*='/album/']").unwrap();
        doc.select(&row_sel)
            .filter_map(|row| {
                let cells = row.select(&cell_sel).map(text).collect();
                let link = row.select(&link_sel).next()?;
                Some((cells, link.value().attr("href")?.to_string(), text(link)))
            })
            .collect()
    };
    let mut books = Vec::new();
    for (cells, href, title) in rows {
        let volume_number = cells.first().and_then(|v| v.parse().ok());
        let publisher = cells.get(2).cloned().unwrap_or_default();
        let publish_date = cells.get(3).cloned();
        let album_url = absolute(base, &href);
        let external_book_id = album_external_id(&album_url).unwrap_or_else(|| album_url.clone());
        let detail = fetch_album(&c, &album_url).await.unwrap_or_default();
        let isbn = detail.get("isbn").cloned();
        let cover_url = detail.get("cover_url").cloned();
        let authors = detail
            .get("authors")
            .map(|s| {
                s.split("\n")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let page_count = detail.get("page_count").and_then(|v| v.parse().ok());
        let summary = detail.get("summary").cloned();
        let date = detail.get("publish_date").cloned().or(publish_date);
        books.push(BookCandidate { external_book_id, title, volume_number, authors, isbn, summary, cover_url, page_count, language: Some("fr".into()), publish_date: date, metadata_json: serde_json::json!({"provider": "bdphile", "publisher": publisher, "source_url": album_url, "oneshot": volume_number.is_none()}) });
    }
    Ok(books)
}

/// Extract a stable album identifier from a BDphile album URL, e.g.
/// "https://www.bdphile.fr/album/bd/138208-les-geants-1-erin" → "bd/138208-les-geants-1-erin".
fn album_external_id(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let path = parsed.path().trim_matches('/');
    path.strip_prefix("album/").map(str::to_string)
}

async fn fetch_album(
    c: &reqwest::Client,
    url: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let body = get_html(c, url).await?;
    let doc = Html::parse_document(&body);
    let mut out = std::collections::HashMap::new();
    if let Some(v) = Selector::parse("meta[name='description']")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .and_then(|e| e.value().attr("content"))
    {
        out.insert("summary".into(), v.to_string());
    }
    if let Some(v) = Selector::parse("meta[property='og:image']")
        .ok()
        .and_then(|s| doc.select(&s).next())
        .and_then(|e| e.value().attr("content"))
    {
        out.insert("cover_url".into(), v.to_string());
    }
    if let Some(v) = field(&doc, "EAN") {
        out.insert("isbn".into(), v);
    }
    if let Some(v) = field(&doc, "Date de publication") {
        out.insert("publish_date".into(), v);
    }
    if let Some(v) = field(&doc, "Format") {
        out.insert(
            "page_count".into(),
            v.split_whitespace()
                .find(|x| x.parse::<i32>().is_ok())
                .unwrap_or("")
                .into(),
        );
    }
    let authors = ["Scénario", "Dessin", "Couleurs", "Couverture"]
        .iter()
        .filter_map(|n| field(&doc, n))
        .collect::<Vec<_>>()
        .join("\n");
    if !authors.is_empty() {
        out.insert("authors".into(), authors);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "tests/bdphile.rs"]
mod tests;
