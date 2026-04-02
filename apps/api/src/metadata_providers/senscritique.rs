use std::collections::HashMap;

use super::{BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

const GRAPHQL_URL: &str = "https://apollo.senscritique.com/";
const POLL_ID_MANGA: i64 = 192836;

pub struct SensCritiqueProvider;

impl MetadataProvider for SensCritiqueProvider {
    fn name(&self) -> &str {
        "senscritique"
    }

    fn search_series(
        &self,
        query: &str,
        _config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<SeriesCandidate>, String>> + Send + '_>,
    > {
        let query = query.to_string();
        Box::pin(async move { search_series_impl(&query).await })
    }

    fn get_series_books(
        &self,
        external_id: &str,
        _config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<BookCandidate>, String>> + Send + '_>,
    > {
        let external_id = external_id.to_string();
        Box::pin(async move { get_series_books_impl(&external_id).await })
    }
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:108.0) Gecko/20100101 Firefox/108.0",
        )
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

// ─── MetadataProvider implementation ─────────────────────────────────────────

/// Search SensCritique for series matching the query.
///
/// Uses `searchAutocomplete` to find products, then deduplicates by franchise
/// so each series appears only once (using the best-rated tome as representative).
async fn search_series_impl(query: &str) -> Result<Vec<SeriesCandidate>, String> {
    let client = build_client()?;

    let gql = serde_json::json!({
        "query": format!(
            r#"{{ searchAutocomplete(keywords: "{}", universe: "comicBook", limit: 20) {{
                items {{
                    product {{
                        id title url category synopsis
                        medias {{ picture }}
                        authors {{ name }}
                        pencillers {{ name }}
                        dateRelease rating yearOfProduction
                        franchises {{ id label }}
                    }}
                }}
            }} }}"#,
            query.replace('"', "\\\"")
        ),
    });

    let data = graphql_request(&client, &gql).await?;

    let items = data
        .pointer("/data/searchAutocomplete/items")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing searchAutocomplete items")?;

    // Group by franchise — keep the first occurrence per franchise
    let mut franchise_map: HashMap<i64, (SeriesCandidate, f64)> = HashMap::new();
    // Track best description per franchise: prefer lowest volume number (tome 1)
    let mut franchise_descriptions: HashMap<i64, (Option<i32>, String)> = HashMap::new();
    // Products without franchise get their own entry
    let mut standalone: Vec<SeriesCandidate> = Vec::new();

    for (i, item) in items.iter().enumerate() {
        let product = match item.get("product") {
            Some(p) => p,
            None => continue,
        };

        let id = product.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
        let title = product
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let rating = product.get("rating").and_then(|r| r.as_f64()).unwrap_or(0.0);

        let franchises = product
            .get("franchises")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default();

        let authors = extract_names(product, "authors");
        let pencillers = extract_names(product, "pencillers");
        let all_authors: Vec<String> = authors
            .into_iter()
            .chain(pencillers)
            .collect::<Vec<_>>();
        // Deduplicate authors
        let mut seen = std::collections::HashSet::new();
        let authors: Vec<String> = all_authors.into_iter().filter(|a| seen.insert(a.clone())).collect();

        let synopsis = product
            .get("synopsis")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let vol_num = extract_volume_number(title);

        let cover_url = product
            .get("medias")
            .and_then(|m| m.get("picture"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.contains("missing"))
            .map(String::from);

        let year = product
            .get("yearOfProduction")
            .and_then(|y| y.as_i64())
            .map(|y| y as i32);

        let url_path = product.get("url").and_then(|v| v.as_str()).unwrap_or_default();
        let external_url = if url_path.is_empty() {
            None
        } else {
            Some(format!("https://www.senscritique.com{url_path}"))
        };

        let confidence = 1.0 - (i as f32 / 20.0).clamp(0.0, 0.9);

        if let Some(franchise) = franchises.first() {
            let fid = franchise.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
            let flabel = franchise
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or(title)
                .to_string();

            let candidate = SeriesCandidate {
                external_id: format!("franchise:{fid}"),
                title: flabel,
                authors,
                description: None,
                publishers: vec![],
                start_year: year,
                total_volumes: None,
                cover_url,
                external_url,
                confidence,
                metadata_json: serde_json::json!({
                    "franchise_id": fid,
                    "rating": rating,
                }),
            };

            // Keep the first occurrence per franchise (most relevant from search order)
            franchise_map.entry(fid).or_insert((candidate, rating));

            // Track description from the lowest volume number (tome 1 preferred)
            if let Some(syn) = synopsis.clone() {
                match franchise_descriptions.get(&fid) {
                    None => { franchise_descriptions.insert(fid, (vol_num, syn)); }
                    Some((existing_vol, _)) => {
                        let new_is_better = match (vol_num, existing_vol) {
                            (Some(n), Some(e)) => n < *e,
                            (Some(_), None) => true,
                            _ => false,
                        };
                        if new_is_better {
                            franchise_descriptions.insert(fid, (vol_num, syn));
                        }
                    }
                }
            }
        } else {
            standalone.push(SeriesCandidate {
                external_id: format!("product:{id}"),
                title: title.to_string(),
                authors,
                description: synopsis,
                publishers: vec![],
                start_year: year,
                total_volumes: None,
                cover_url,
                external_url,
                confidence,
                metadata_json: serde_json::json!({
                    "product_id": id,
                    "rating": rating,
                }),
            });
        }
    }

    // Merge: franchises first (sorted by confidence), then standalone
    // Apply tome 1 descriptions to franchise candidates
    let mut results: Vec<SeriesCandidate> = franchise_map
        .into_iter()
        .map(|(fid, (mut c, _))| {
            if c.description.is_none() {
                if let Some((_, desc)) = franchise_descriptions.remove(&fid) {
                    c.description = Some(desc);
                }
            }
            c
        })
        .collect();
    results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    results.extend(standalone);

    Ok(results)
}

/// Get all volumes/books for a series identified by its external_id.
///
/// external_id format: "franchise:{id}" or "product:{id}"
async fn get_series_books_impl(external_id: &str) -> Result<Vec<BookCandidate>, String> {
    let client = build_client()?;

    if let Some(fid_str) = external_id.strip_prefix("franchise:") {
        let fid: i64 = fid_str
            .parse()
            .map_err(|_| format!("invalid franchise id: {fid_str}"))?;
        fetch_franchise_books(&client, fid).await
    } else if let Some(pid_str) = external_id.strip_prefix("product:") {
        let pid: i64 = pid_str
            .parse()
            .map_err(|_| format!("invalid product id: {pid_str}"))?;
        // Single product — return it as a single book
        fetch_single_book(&client, pid).await
    } else {
        Err(format!("unrecognized external_id format: {external_id}"))
    }
}

/// Fetch all books in a franchise via groupProducts.
async fn fetch_franchise_books(client: &reqwest::Client, franchise_id: i64) -> Result<Vec<BookCandidate>, String> {
    let gql = serde_json::json!({
        "query": format!(
            r#"{{ groupProducts(franchiseId: {franchise_id}, universe: "comicBook", limit: 200, offset: 0) {{
                items {{
                    id title url dateRelease rating
                    medias {{ picture }}
                    authors {{ name }}
                    synopsis
                }}
            }} }}"#
        ),
    });

    let data = graphql_request(client, &gql).await?;

    let items = data
        .pointer("/data/groupProducts/items")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing groupProducts items")?;

    // Parse all items, then deduplicate by volume number (keep earliest edition)
    let mut all_books: Vec<BookCandidate> = items
        .iter()
        .filter_map(|product| {
            let id = product.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
            let title = product
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if title.is_empty() {
                return None;
            }

            let volume_number = extract_volume_number(&title);

            let cover_url = product
                .get("medias")
                .and_then(|m| m.get("picture"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.contains("missing"))
                .map(String::from);

            let summary = product
                .get("synopsis")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);

            let publish_date = product
                .get("dateRelease")
                .and_then(|d| d.as_str())
                .map(String::from);

            let authors = extract_names(product, "authors");

            Some(BookCandidate {
                external_book_id: id.to_string(),
                title,
                volume_number,
                authors,
                isbn: None,
                summary,
                cover_url,
                page_count: None,
                language: Some("fr".to_string()),
                publish_date,
                metadata_json: serde_json::json!({}),
            })
        })
        .collect();

    // Sort by publish date (oldest first) so dedup keeps the original edition
    all_books.sort_by(|a, b| a.publish_date.cmp(&b.publish_date));

    // Deduplicate by volume number: keep earliest edition per volume
    let mut seen_volumes: HashMap<i32, usize> = HashMap::new();
    let mut deduped: Vec<BookCandidate> = Vec::new();
    for book in all_books {
        if let Some(vol) = book.volume_number {
            if let std::collections::hash_map::Entry::Vacant(e) = seen_volumes.entry(vol) {
                e.insert(deduped.len());
                deduped.push(book);
            }
            // Skip duplicate volume numbers (later editions)
        } else {
            // Books without volume number (intégrales, artbooks, etc.) — skip
            // to avoid polluting the volume list
        }
    }

    // Sort by volume number
    deduped.sort_by_key(|b| b.volume_number.unwrap_or(i32::MAX));

    Ok(deduped)
}

/// Fetch a single product as a book.
async fn fetch_single_book(client: &reqwest::Client, product_id: i64) -> Result<Vec<BookCandidate>, String> {
    let gql = serde_json::json!({
        "query": format!(
            r#"{{ product(id: {product_id}) {{
                id title url dateRelease rating
                medias {{ picture }}
                authors {{ name }}
                synopsis
            }} }}"#
        ),
    });

    let data = graphql_request(client, &gql).await?;

    let product = data
        .pointer("/data/product")
        .ok_or("SensCritique: product not found")?;

    let id = product.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
    let title = product
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let cover_url = product
        .get("medias")
        .and_then(|m| m.get("picture"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.contains("missing"))
        .map(String::from);

    let summary = product
        .get("synopsis")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let publish_date = product
        .get("dateRelease")
        .and_then(|d| d.as_str())
        .map(String::from);

    let authors = extract_names(product, "authors");

    Ok(vec![BookCandidate {
        external_book_id: id.to_string(),
        title,
        volume_number: Some(1),
        authors,
        isbn: None,
        summary,
        cover_url,
        page_count: None,
        language: Some("fr".to_string()),
        publish_date,
        metadata_json: serde_json::json!({}),
    }])
}

// ─── Discovery functions (used by discovery.rs) ─────────────────────────────

/// Fetch the SensCritique "meilleurs mangas" top list via GraphQL.
pub async fn fetch_top_mangas(limit: usize) -> Result<Vec<SeriesCandidate>, String> {
    let query = serde_json::json!({
        "query": "{ poll(id: POLL_ID, limit: LIMIT, offset: 0) { products { id title url medias { picture } authors { name } dateRelease rating } } }"
            .replace("POLL_ID", &POLL_ID_MANGA.to_string())
            .replace("LIMIT", &limit.to_string()),
    });

    let client = build_client()?;
    let data = graphql_request(&client, &query).await?;

    let items = data
        .pointer("/data/poll/products")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing poll products in GraphQL response")?;

    Ok(parse_products(items, limit))
}

/// Fetch the SensCritique "top 100 BD" list via GraphQL.
pub async fn fetch_top_bd(limit: usize) -> Result<Vec<SeriesCandidate>, String> {
    let query = serde_json::json!({
        "query": format!(
            "{{ top(universe: \"comicBook\", subtype: TOP_100_OUT_OF_TOP_10, limit: {limit}, offset: 0) {{ id title url medias {{ picture }} authors {{ name }} dateRelease rating }} }}"
        ),
    });

    let client = build_client()?;
    let data = graphql_request(&client, &query).await?;

    let items = data
        .pointer("/data/top")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing top in GraphQL response")?;

    Ok(parse_products(items, limit))
}

/// Fetch trending/new releases from SensCritique.
pub async fn fetch_trending(
    universe: &str,
    period: &str,
    sort_by: &str,
    category: Option<&str>,
    limit: usize,
) -> Result<Vec<SeriesCandidate>, String> {
    let fetch_limit = if category.is_some() { limit * 4 } else { limit };

    let query = serde_json::json!({
        "query": format!(
            "{{ productsByRelease(universe: \"{universe}\", limit: {fetch_limit}, offset: 0, sortBy: {sort_by}, byPeriod: true, period: {period}) {{ items {{ id title url category medias {{ picture }} authors {{ name }} dateRelease rating }} }} }}"
        ),
    });

    let client = build_client()?;
    let data = graphql_request(&client, &query).await?;

    let items = data
        .pointer("/data/productsByRelease/items")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing productsByRelease items in GraphQL response")?;

    let mut candidates = parse_products(items, fetch_limit);

    if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        candidates.retain(|c| {
            c.metadata_json
                .get("category")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase() == cat_lower)
                .unwrap_or(false)
        });
        candidates.truncate(limit);
    } else {
        candidates.truncate(limit);
    }

    Ok(candidates)
}

// ─── Shared helpers ─────────────────────────────────────────────────────────

async fn graphql_request(
    client: &reqwest::Client,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let resp = client
        .post(GRAPHQL_URL)
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| format!("SensCritique GraphQL request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("SensCritique GraphQL returned HTTP {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("SensCritique GraphQL parse failed: {e}"))?;

    if let Some(errors) = data.get("errors").and_then(|e| e.as_array()) {
        if let Some(first) = errors.first() {
            let msg = first
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(format!("SensCritique GraphQL error: {msg}"));
        }
    }

    Ok(data)
}

fn parse_products(items: &[serde_json::Value], limit: usize) -> Vec<SeriesCandidate> {
    items
        .iter()
        .take(limit)
        .enumerate()
        .filter_map(|(i, product)| {
            let id = product.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
            let title = product
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if title.is_empty() {
                return None;
            }

            let url_path = product
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let external_url = if url_path.is_empty() {
                None
            } else {
                Some(format!("https://www.senscritique.com{url_path}"))
            };

            let cover_url = product
                .get("medias")
                .and_then(|m| m.get("picture"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.contains("missing"))
                .map(String::from);

            let authors = extract_names(product, "authors");

            let year = product
                .get("dateRelease")
                .and_then(|d| d.as_str())
                .and_then(|d| d.split('-').next())
                .and_then(|y| y.parse::<i32>().ok());

            let rating = product.get("rating").and_then(|r| r.as_f64());
            let category = product
                .get("category")
                .and_then(|c| c.as_str())
                .map(String::from);

            let confidence = 1.0 - (i as f32 / 100.0).clamp(0.0, 0.9);

            Some(SeriesCandidate {
                external_id: id.to_string(),
                title,
                authors,
                description: None,
                publishers: vec![],
                start_year: year,
                total_volumes: None,
                cover_url,
                external_url,
                confidence,
                metadata_json: serde_json::json!({
                    "source": "senscritique",
                    "rating": rating,
                    "category": category,
                }),
            })
        })
        .collect()
}

fn extract_names(product: &serde_json::Value, field: &str) -> Vec<String> {
    product
        .get(field)
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract volume number from title patterns like "..., tome 3" or "... T.3"
fn extract_volume_number(title: &str) -> Option<i32> {
    use std::sync::LazyLock;
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)(?:tome|t\.|vol(?:ume)?\.?)\s*(\d+)").unwrap()
    });

    RE.captures(title)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse().ok())
}
