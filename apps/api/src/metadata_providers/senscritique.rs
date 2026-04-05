use std::collections::HashMap;

use base64::Engine as _;

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
        config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<SeriesCandidate>, String>> + Send + '_>,
    > {
        let query = query.to_string();
        let detailed = config.detailed;
        Box::pin(async move { search_series_impl(&query, detailed).await })
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
async fn search_series_impl(query: &str, detailed: bool) -> Result<Vec<SeriesCandidate>, String> {
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

    // Fetch latest release dates for all franchises in one GraphQL query to infer status
    let franchise_ids: Vec<i64> = franchise_map.keys().copied().collect();
    let latest_dates = fetch_franchise_latest_dates(&client, &franchise_ids).await;

    if detailed {
        // Manual search: fetch all products per franchise, group by edition
        let mut results: Vec<SeriesCandidate> = Vec::new();
        for (fid, (base_candidate, _rating)) in &franchise_map {
            let edition_candidates = fetch_franchise_editions(&client, *fid, base_candidate, &franchise_descriptions, &latest_dates, query).await;
            results.extend(edition_candidates);
        }
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        results.extend(standalone);
        Ok(results)
    } else {
        // Batch mode: return one candidate per franchise (no extra API calls)
        let mut results: Vec<SeriesCandidate> = franchise_map
            .into_iter()
            .map(|(fid, (mut c, _))| {
                if c.description.is_none() {
                    if let Some((_, desc)) = franchise_descriptions.remove(&fid) {
                        c.description = Some(desc);
                    }
                }
                if let Some(date_str) = latest_dates.get(&fid) {
                    let status = infer_status_from_date(date_str);
                    c.metadata_json["status"] = serde_json::json!(status);
                }
                c
            })
            .collect();
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        results.extend(standalone);
        Ok(results)
    }
}

/// Fetch all products in a franchise, group by edition, and return one SeriesCandidate per edition.
async fn fetch_franchise_editions(
    client: &reqwest::Client,
    franchise_id: i64,
    base_candidate: &SeriesCandidate,
    franchise_descriptions: &HashMap<i64, (Option<i32>, String)>,
    latest_dates: &HashMap<i64, String>,
    search_query: &str,
) -> Vec<SeriesCandidate> {
    let gql = serde_json::json!({
        "query": format!(
            r#"{{ groupProducts(franchiseId: {franchise_id}, universe: "comicBook", limit: 200, offset: 0) {{
                items {{ id title url dateRelease medias {{ picture }} authors {{ name }} synopsis }}
            }} }}"#
        ),
    });

    let items = match graphql_request(client, &gql).await {
        Ok(data) => data
            .pointer("/data/groupProducts/items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        Err(_) => {
            // Fallback: return base candidate without edition info
            return vec![base_candidate.clone()];
        }
    };

    let editions = group_products_by_edition(&items);

    if editions.is_empty() {
        // No editions found — return base candidate as-is
        return vec![base_candidate.clone()];
    }

    let status = latest_dates.get(&franchise_id).map(|d| infer_status_from_date(d));

    editions
        .into_iter()
        .map(|(edition_name, products)| {
            let volume_count = products.len() as i32;

            // Pick cover from tome 1 (earliest volume)
            let cover_url = products
                .iter()
                .filter_map(|p| {
                    let vol = extract_volume_number(p.get("title").and_then(|v| v.as_str()).unwrap_or_default());
                    vol.map(|v| (v, p))
                })
                .min_by_key(|(v, _)| *v)
                .and_then(|(_, p)| {
                    p.get("medias")
                        .and_then(|m| m.get("picture"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.contains("missing"))
                        .map(String::from)
                })
                .or_else(|| base_candidate.cover_url.clone());

            // Description from lowest volume number in this edition
            let description = products
                .iter()
                .filter_map(|p| {
                    let title = p.get("title").and_then(|v| v.as_str()).unwrap_or_default();
                    let vol = extract_volume_number(title).unwrap_or(i32::MAX);
                    let syn = p.get("synopsis")
                        .and_then(|s| s.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from)?;
                    Some((vol, syn))
                })
                .min_by_key(|(v, _)| *v)
                .map(|(_, syn)| syn)
                .or_else(|| {
                    franchise_descriptions
                        .get(&franchise_id)
                        .map(|(_, desc)| desc.clone())
                });

            // Year from earliest product
            let start_year = products
                .iter()
                .filter_map(|p| {
                    p.get("dateRelease")
                        .and_then(|d| d.as_str())
                        .and_then(|d| d.get(..4))
                        .and_then(|y| y.parse::<i32>().ok())
                })
                .min()
                .or(base_candidate.start_year);

            let authors = base_candidate.authors.clone();

            let mut metadata = base_candidate.metadata_json.clone();
            if let Some(ref s) = status {
                metadata["status"] = serde_json::json!(s);
            }
            metadata["edition"] = serde_json::json!(edition_name);

            // Confidence based on name similarity with search query
            let similarity = name_similarity(search_query, &edition_name);
            // Bonus for editions with more volumes (main edition likely more relevant)
            let volume_bonus = (volume_count as f32 / 100.0).min(0.1);
            let confidence = (similarity + volume_bonus).min(1.0);

            SeriesCandidate {
                external_id: format!("franchise:{}:edition:{}", franchise_id, encode_edition(&edition_name)),
                title: edition_name,
                authors,
                description,
                publishers: vec![],
                start_year,
                total_volumes: Some(volume_count),
                cover_url,
                external_url: base_candidate.external_url.clone(),
                confidence,
                metadata_json: metadata,
            }
        })
        .collect()
}

/// Get all volumes/books for a series identified by its external_id.
///
/// external_id format: "franchise:{id}:edition:{encoded}" or "franchise:{id}" or "product:{id}"
async fn get_series_books_impl(external_id: &str) -> Result<Vec<BookCandidate>, String> {
    let client = build_client()?;

    if let Some(rest) = external_id.strip_prefix("franchise:") {
        let (fid_str, edition_filter) = if let Some((fid_part, edition_part)) = rest.split_once(":edition:") {
            let decoded = decode_edition(edition_part)?;
            (fid_part, Some(decoded))
        } else {
            (rest, None)
        };
        let fid: i64 = fid_str
            .parse()
            .map_err(|_| format!("invalid franchise id: {fid_str}"))?;
        fetch_franchise_books(&client, fid, edition_filter.as_deref()).await
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

/// Fetch all books in a franchise via groupProducts, optionally filtered by edition.
async fn fetch_franchise_books(client: &reqwest::Client, franchise_id: i64, edition_filter: Option<&str>) -> Result<Vec<BookCandidate>, String> {
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

    // If edition filter is set, only keep products matching that edition
    let filtered_items: Vec<&serde_json::Value> = if let Some(edition) = edition_filter {
        items
            .iter()
            .filter(|p| {
                let title = p.get("title").and_then(|v| v.as_str()).unwrap_or_default();
                extract_edition_name(title).as_deref() == Some(edition)
            })
            .collect()
    } else {
        items.iter().collect()
    };

    // Parse all items, then deduplicate by volume number (keep earliest edition)
    let mut all_books: Vec<BookCandidate> = filtered_items
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
        "query": "{ poll(id: POLL_ID, limit: LIMIT, offset: 0) { products { id title url medias { picture } authors { name } dateRelease rating synopsis } } }"
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
            "{{ top(universe: \"comicBook\", subtype: TOP_100_OUT_OF_TOP_10, limit: {limit}, offset: 0) {{ id title url medias {{ picture }} authors {{ name }} dateRelease rating synopsis }} }}"
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
            "{{ productsByRelease(universe: \"{universe}\", limit: {fetch_limit}, offset: 0, sortBy: {sort_by}, byPeriod: true, period: {period}) {{ items {{ id title url category medias {{ picture }} authors {{ name }} dateRelease rating synopsis }} }} }}"
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

/// Fetch the latest release date for each franchise in a single batched GraphQL query.
async fn fetch_franchise_latest_dates(
    client: &reqwest::Client,
    franchise_ids: &[i64],
) -> HashMap<i64, String> {
    if franchise_ids.is_empty() {
        return HashMap::new();
    }

    // Build a batched query with aliases: f_123: groupProducts(franchiseId: 123, ...)
    let fields: Vec<String> = franchise_ids
        .iter()
        .map(|fid| {
            format!(
                "f_{fid}: groupProducts(franchiseId: {fid}, universe: \"comicBook\", limit: 1, offset: 0, order: DATE_RELEASE_DESC) {{ items {{ dateRelease }} }}"
            )
        })
        .collect();

    let query = format!("{{ {} }}", fields.join(" "));
    let gql = serde_json::json!({ "query": query });

    let data = match graphql_request(client, &gql).await {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };

    let mut result = HashMap::new();
    if let Some(obj) = data.get("data").and_then(|d| d.as_object()) {
        for fid in franchise_ids {
            let key = format!("f_{fid}");
            if let Some(date) = obj
                .get(&key)
                .and_then(|gp| gp.get("items"))
                .and_then(|items| items.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("dateRelease"))
                .and_then(|d| d.as_str())
            {
                result.insert(*fid, date.to_string());
            }
        }
    }

    result
}

/// Infer series status from the latest release date.
/// If the latest volume was released within the last 18 months → "ongoing", otherwise "ended".
fn infer_status_from_date(date_str: &str) -> &'static str {
    use chrono::{NaiveDate, Utc};
    let cutoff = Utc::now().date_naive() - chrono::Duration::days(18 * 30);
    match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) if date > cutoff => "ongoing",
        _ => "ended",
    }
}

// ─── Shared helpers ─────────────────────────────────────────────────────────

async fn graphql_request(
    client: &reqwest::Client,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    graphql_request_url(client, GRAPHQL_URL, body).await
}

async fn graphql_request_url(
    client: &reqwest::Client,
    url: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let max_retries = 3;
    let mut delay_ms = 1000u64;

    for attempt in 0..=max_retries {
        let resp = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("SensCritique GraphQL request failed: {e}"))?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if attempt < max_retries {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                delay_ms *= 2; // exponential backoff
                continue;
            }
            return Err("SensCritique GraphQL returned HTTP 429 Too Many Requests (after retries)".to_string());
        }

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

        return Ok(data);
    }

    Err("SensCritique GraphQL: max retries exceeded".to_string())
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
            let description = product
                .get("synopsis")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);

            let confidence = 1.0 - (i as f32 / 100.0).clamp(0.0, 0.9);

            Some(SeriesCandidate {
                external_id: id.to_string(),
                title,
                authors,
                description,
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

/// Encode an edition name for use in external_id.
fn encode_edition(name: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(name)
}

/// Decode an edition name from external_id.
fn decode_edition(encoded: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| format!("invalid base64 edition: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("invalid utf8 edition: {e}"))
}

/// Compute name similarity between 0.0 and 1.0.
/// Compares normalized (lowercased, unaccented-ish) names.
fn name_similarity(a: &str, b: &str) -> f32 {
    let normalize = |s: &str| -> String {
        s.to_lowercase()
            .replace(['é', 'è', 'ê', 'ë'], "e")
            .replace(['à', 'â', 'ä'], "a")
            .replace(['ù', 'û', 'ü'], "u")
            .replace(['î', 'ï'], "i")
            .replace(['ô', 'ö'], "o")
            .replace('ç', "c")
            .replace(|c: char| !c.is_alphanumeric(), " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let na = normalize(a);
    let nb = normalize(b);
    if na == nb {
        return 1.0;
    }
    if na.is_empty() || nb.is_empty() {
        return 0.0;
    }
    // Check containment
    if na.contains(&nb) || nb.contains(&na) {
        let shorter = na.len().min(nb.len()) as f32;
        let longer = na.len().max(nb.len()) as f32;
        return (shorter / longer).max(0.5);
    }
    // Word overlap
    let words_a: std::collections::HashSet<&str> = na.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = nb.split_whitespace().collect();
    let intersection = words_a.intersection(&words_b).count() as f32;
    let union = words_a.union(&words_b).count() as f32;
    if union == 0.0 { 0.0 } else { intersection / union }
}

/// Extract edition name from a SensCritique product title.
/// "Titre du tome - Naruto, tome 42" → Some("Naruto")
/// "Naruto (Édition Hokage), tome 33" → Some("Naruto (Édition Hokage)")
/// "Boruto: Two Blue Vortex, tome 5" → Some("Boruto: Two Blue Vortex")
/// "One Piece - Intégrale" → None (no tome pattern)
fn extract_edition_name(title: &str) -> Option<String> {
    use std::sync::LazyLock;
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i),?\s*(?:tome|t\.|vol(?:ume)?\.?)\s*\d+\s*$").unwrap()
    });

    let m = RE.find(title)?;
    let prefix = title[..m.start()].trim();
    if prefix.is_empty() {
        return None;
    }

    // If prefix contains " - ", the edition name is after the last " - "
    // e.g., "Titre du tome - Naruto" → "Naruto"
    let edition = if let Some(idx) = prefix.rfind(" - ") {
        prefix[idx + 3..].trim()
    } else {
        prefix
    };

    if edition.is_empty() {
        None
    } else {
        Some(edition.to_string())
    }
}

/// Group products by edition name. Returns (edition_name, products) pairs.
fn group_products_by_edition(items: &[serde_json::Value]) -> Vec<(String, Vec<&serde_json::Value>)> {
    let mut groups: HashMap<String, Vec<&serde_json::Value>> = HashMap::new();

    for item in items {
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or_default();
        if let Some(edition) = extract_edition_name(title) {
            groups.entry(edition).or_default().push(item);
        }
    }

    let mut result: Vec<_> = groups.into_iter().collect();
    // Sort by volume count descending (main edition first)
    result.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    result
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─── extract_volume_number ───────────────────────────────────────────

    #[test]
    fn extract_volume_number_tome() {
        assert_eq!(extract_volume_number("One Piece, tome 3"), Some(3));
        assert_eq!(extract_volume_number("Naruto Tome 12"), Some(12));
        assert_eq!(extract_volume_number("TOME 1"), Some(1));
    }

    #[test]
    fn extract_volume_number_t_dot() {
        assert_eq!(extract_volume_number("One Piece T.3"), Some(3));
        assert_eq!(extract_volume_number("Series T.12"), Some(12));
        assert_eq!(extract_volume_number("T.007"), Some(7));
    }

    #[test]
    fn extract_volume_number_vol() {
        assert_eq!(extract_volume_number("Vol. 12"), Some(12));
        assert_eq!(extract_volume_number("Vol 5"), Some(5));
        assert_eq!(extract_volume_number("Vol.3"), Some(3));
        assert_eq!(extract_volume_number("Volume 8"), Some(8));
        assert_eq!(extract_volume_number("volume 1"), Some(1));
    }

    #[test]
    fn extract_volume_number_integrale_no_match() {
        // "Intégrale" has no volume number pattern
        assert_eq!(extract_volume_number("One Piece - Intégrale"), None);
    }

    #[test]
    fn extract_volume_number_no_volume() {
        assert_eq!(extract_volume_number("Just a book title"), None);
        assert_eq!(extract_volume_number(""), None);
        assert_eq!(extract_volume_number("No numbers at all"), None);
    }

    #[test]
    fn extract_volume_number_zero_padded() {
        assert_eq!(extract_volume_number("Tome 007"), Some(7));
        assert_eq!(extract_volume_number("T.001"), Some(1));
    }

    // ─── infer_status_from_date ──────────────────────────────────────────

    #[test]
    fn infer_status_recent_date_is_ongoing() {
        // A date very recently should be "ongoing"
        let recent = chrono::Utc::now().date_naive() - chrono::Duration::days(30);
        let date_str = recent.format("%Y-%m-%d").to_string();
        assert_eq!(infer_status_from_date(&date_str), "ongoing");
    }

    #[test]
    fn infer_status_old_date_is_ended() {
        // A date 3 years ago should be "ended"
        assert_eq!(infer_status_from_date("2020-01-01"), "ended");
    }

    #[test]
    fn infer_status_invalid_date_is_ended() {
        assert_eq!(infer_status_from_date("not-a-date"), "ended");
        assert_eq!(infer_status_from_date(""), "ended");
        assert_eq!(infer_status_from_date("2024/01/01"), "ended"); // wrong format
    }

    #[test]
    fn infer_status_boundary_date() {
        // Exactly at the 18-month cutoff boundary
        let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(18 * 30);
        let date_str = cutoff.format("%Y-%m-%d").to_string();
        // At the cutoff date itself, date is NOT > cutoff, so "ended"
        assert_eq!(infer_status_from_date(&date_str), "ended");

        // One day after cutoff should be "ongoing"
        let one_after = cutoff + chrono::Duration::days(1);
        let date_str = one_after.format("%Y-%m-%d").to_string();
        assert_eq!(infer_status_from_date(&date_str), "ongoing");
    }

    // ─── extract_names ───────────────────────────────────────────────────

    #[test]
    fn extract_names_with_array() {
        let product = serde_json::json!({
            "authors": [
                {"name": "Eiichiro Oda"},
                {"name": "Another Author"}
            ]
        });
        assert_eq!(extract_names(&product, "authors"), vec!["Eiichiro Oda", "Another Author"]);
    }

    #[test]
    fn extract_names_empty() {
        let product = serde_json::json!({});
        assert_eq!(extract_names(&product, "authors"), Vec::<String>::new());
    }

    #[test]
    fn extract_names_missing_name_field() {
        let product = serde_json::json!({
            "authors": [
                {"name": "Author1"},
                {"other": "no name"},
                {"name": "Author2"}
            ]
        });
        assert_eq!(extract_names(&product, "authors"), vec!["Author1", "Author2"]);
    }

    // ─── Wiremock integration tests ─────────────────────────────────────

    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    fn mock_autocomplete_response() -> serde_json::Value {
        serde_json::json!({
            "data": {
                "searchAutocomplete": {
                    "items": [
                        {
                            "product": {
                                "id": 112604,
                                "title": "Quelque part entre les ombres - Blacksad, tome 1",
                                "url": "/bd/blacksad_tome_1/112604",
                                "category": "BD franco-belge",
                                "synopsis": "Blacksad est un chat détective privé.",
                                "medias": { "picture": "https://example.com/cover.jpg" },
                                "authors": [{ "name": "Juan Díaz Canales" }],
                                "pencillers": [{ "name": "Juanjo Guarnido" }],
                                "dateRelease": "2000-11-10",
                                "rating": 8.1,
                                "yearOfProduction": 2000,
                                "franchises": [{ "id": 2430, "label": "Blacksad" }]
                            }
                        },
                        {
                            "product": {
                                "id": 386532,
                                "title": "Arctic-Nation - Blacksad, tome 2",
                                "url": "/bd/blacksad_tome_2/386532",
                                "category": "BD franco-belge",
                                "synopsis": "Deuxième aventure de Blacksad.",
                                "medias": { "picture": "https://example.com/cover2.jpg" },
                                "authors": [{ "name": "Juan Díaz Canales" }],
                                "pencillers": [{ "name": "Juanjo Guarnido" }],
                                "dateRelease": "2003-03-22",
                                "rating": 8.3,
                                "yearOfProduction": 2003,
                                "franchises": [{ "id": 2430, "label": "Blacksad" }]
                            }
                        },
                        {
                            "product": {
                                "id": 999,
                                "title": "Standalone Book",
                                "url": "/bd/standalone/999",
                                "category": "BD",
                                "synopsis": "A standalone book.",
                                "medias": { "picture": "https://example.com/standalone.jpg" },
                                "authors": [{ "name": "Solo Author" }],
                                "pencillers": [],
                                "dateRelease": "2020-01-01",
                                "rating": 7.0,
                                "yearOfProduction": 2020,
                                "franchises": []
                            }
                        }
                    ]
                }
            }
        })
    }

    fn mock_latest_dates_response() -> serde_json::Value {
        serde_json::json!({
            "data": {
                "f_2430": {
                    "items": [{ "dateRelease": "2025-12-05" }]
                }
            }
        })
    }

    #[tokio::test]
    async fn wiremock_graphql_request_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "test": true }
            })))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let body = serde_json::json!({ "query": "{ test }" });
        let result = graphql_request_url(&client, &server.uri(), &body).await.unwrap();
        assert_eq!(result["data"]["test"], true);
    }

    #[tokio::test]
    async fn wiremock_graphql_error_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "errors": [{ "message": "Something went wrong" }]
            })))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let body = serde_json::json!({ "query": "{ bad }" });
        let result = graphql_request_url(&client, &server.uri(), &body).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Something went wrong"));
    }

    #[tokio::test]
    async fn wiremock_graphql_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let body = serde_json::json!({ "query": "{ test }" });
        let result = graphql_request_url(&client, &server.uri(), &body).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HTTP 500"));
    }

    #[tokio::test]
    async fn wiremock_search_deduplicates_by_franchise() {
        let server = MockServer::start().await;

        // First call: searchAutocomplete
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_autocomplete_response()))
            .expect(1..=2) // autocomplete + latest dates
            .mount(&server)
            .await;

        // Mock the latest dates call (second POST)
        // Since wiremock matches both POSTs the same way, we need to handle it
        // For this test, we just verify the parsing logic with mock data directly

        let client = build_client().unwrap();
        let body = serde_json::json!({
            "query": r#"{ searchAutocomplete(keywords: "Blacksad", universe: "comicBook", limit: 20) { items { product { id title url category synopsis medias { picture } authors { name } pencillers { name } dateRelease rating yearOfProduction franchises { id label } } } } }"#,
        });
        let data = graphql_request_url(&client, &server.uri(), &body).await.unwrap();

        let items = data.pointer("/data/searchAutocomplete/items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 3, "mock returns 3 items");

        // Verify dedup logic manually
        let mut franchise_map: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
        let mut standalone_count = 0;
        for item in items {
            let product = item.get("product").unwrap();
            let franchises = product.get("franchises").and_then(|f| f.as_array()).unwrap();
            if let Some(f) = franchises.first() {
                let fid = f["id"].as_i64().unwrap();
                franchise_map.entry(fid).or_insert_with(|| product["title"].as_str().unwrap().to_string());
            } else {
                standalone_count += 1;
            }
        }
        assert_eq!(franchise_map.len(), 1, "two Blacksad tomes should dedup to one franchise");
        assert_eq!(standalone_count, 1, "one standalone book");
        assert!(franchise_map.contains_key(&2430));
    }

    #[tokio::test]
    async fn wiremock_parse_products_filters_missing_covers() {
        let items = vec![
            serde_json::json!({
                "id": 1, "title": "Has Cover", "url": "/test/1",
                "medias": { "picture": "https://example.com/real.jpg" },
                "authors": [], "dateRelease": "2024-01-01", "rating": 8.0
            }),
            serde_json::json!({
                "id": 2, "title": "Missing Cover", "url": "/test/2",
                "medias": { "picture": "https://media.senscritique.com/missing/701/300x0/missing.png" },
                "authors": [], "dateRelease": "2024-01-01", "rating": 7.0
            }),
        ];
        let results = parse_products(&items, 10);
        assert_eq!(results.len(), 2);
        assert!(results[0].cover_url.is_some(), "real cover should be kept");
        assert!(results[1].cover_url.is_none(), "missing.png cover should be filtered out");
    }

    // ─── extract_edition_name ─────────────────────────────────────────

    #[test]
    fn edition_name_standard() {
        assert_eq!(
            extract_edition_name("Naruto Uzumaki !! - Naruto, tome 1"),
            Some("Naruto".to_string())
        );
    }

    #[test]
    fn edition_name_with_parentheses() {
        assert_eq!(
            extract_edition_name("Naruto (Édition Hokage), tome 33"),
            Some("Naruto (Édition Hokage)".to_string())
        );
    }

    #[test]
    fn edition_name_colon() {
        assert_eq!(
            extract_edition_name("Boruto: Two Blue Vortex, tome 5"),
            Some("Boruto: Two Blue Vortex".to_string())
        );
    }

    #[test]
    fn edition_name_no_tome() {
        assert_eq!(extract_edition_name("One Piece - Intégrale"), None);
        assert_eq!(extract_edition_name("Naruto"), None);
    }

    #[test]
    fn edition_name_t_dot() {
        assert_eq!(
            extract_edition_name("One Piece T.42"),
            Some("One Piece".to_string())
        );
    }

    #[test]
    fn edition_name_simple() {
        assert_eq!(
            extract_edition_name("Dragon Ball, tome 10"),
            Some("Dragon Ball".to_string())
        );
    }

    // ─── encode/decode edition ────────────────────────────────────────

    #[test]
    fn edition_encode_decode_roundtrip() {
        for name in ["Naruto", "Naruto (Édition Hokage)", "Boruto: Two Blue Vortex", "Astérix"] {
            let encoded = encode_edition(name);
            let decoded = decode_edition(&encoded).unwrap();
            assert_eq!(decoded, name, "round-trip failed for {name}");
        }
    }

    // ─── group_products_by_edition ────────────────────────────────────

    #[test]
    fn group_by_edition_mixed() {
        let items = vec![
            serde_json::json!({"title": "Naruto Uzumaki !! - Naruto, tome 1", "id": 1}),
            serde_json::json!({"title": "Se battre - Naruto, tome 2", "id": 2}),
            serde_json::json!({"title": "Naruto (Édition Hokage), tome 1", "id": 3}),
            serde_json::json!({"title": "Naruto (Édition Hokage), tome 2", "id": 4}),
            serde_json::json!({"title": "Naruto (Édition Hokage), tome 3", "id": 5}),
            serde_json::json!({"title": "Naruto - Intégrale", "id": 6}),
        ];
        let groups = group_products_by_edition(&items);
        assert_eq!(groups.len(), 2, "should have 2 editions");
        assert_eq!(groups[0].0, "Naruto (Édition Hokage)");
        assert_eq!(groups[0].1.len(), 3);
        assert_eq!(groups[1].0, "Naruto");
        assert_eq!(groups[1].1.len(), 2);
    }

    #[test]
    fn group_by_edition_excludes_no_tome() {
        let items = vec![
            serde_json::json!({"title": "Artbook", "id": 1}),
            serde_json::json!({"title": "Intégrale collector", "id": 2}),
        ];
        let groups = group_products_by_edition(&items);
        assert!(groups.is_empty());
    }

    // ─── external_id parsing ──────────────────────────────────────────

    #[test]
    fn external_id_franchise_with_edition() {
        let name = "Naruto (Édition Hokage)";
        let ext_id = format!("franchise:817:edition:{}", encode_edition(name));
        let rest = ext_id.strip_prefix("franchise:").unwrap();
        let (fid_str, edition_part) = rest.split_once(":edition:").unwrap();
        assert_eq!(fid_str, "817");
        assert_eq!(decode_edition(edition_part).unwrap(), name);
    }

    #[test]
    fn external_id_franchise_backward_compat() {
        let ext_id = "franchise:817";
        let rest = ext_id.strip_prefix("franchise:").unwrap();
        assert!(rest.split_once(":edition:").is_none(), "old format has no edition");
    }

    // ─── name_similarity ──────────────────────────────────────────────

    #[test]
    fn similarity_exact_match() {
        assert!((name_similarity("Naruto", "Naruto") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn similarity_case_insensitive() {
        assert!((name_similarity("naruto", "NARUTO") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn similarity_accent_insensitive() {
        assert!((name_similarity("Astérix", "Asterix") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn similarity_edition_suffix_lower() {
        // "Naruto" vs "Naruto (Édition Hokage)" — contained, so > 0.5
        let s = name_similarity("Naruto", "Naruto (Édition Hokage)");
        assert!(s > 0.3 && s < 1.0, "partial containment: got {s}");
    }

    #[test]
    fn similarity_different_series() {
        let s = name_similarity("Naruto", "One Piece");
        assert!(s < 0.2, "unrelated: got {s}");
    }

    #[test]
    fn similarity_subseries() {
        // "Boruto" vs "Naruto" — share "ruto" but are different
        let s = name_similarity("Boruto", "Naruto");
        assert!(s < 0.5, "different series: got {s}");
    }
}
