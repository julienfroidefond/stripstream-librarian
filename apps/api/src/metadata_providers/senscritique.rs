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

    fn get_series(
        &self,
        external_id: &str,
        _config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<SeriesCandidate, String>> + Send + '_>,
    > {
        let external_id = external_id.to_string();
        Box::pin(async move { get_series_impl(&external_id).await })
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
    let mut franchise_map: HashMap<i64, (SeriesCandidate, Option<f64>)> = HashMap::new();
    // Track best description per franchise: prefer lowest volume number (tome 1)
    let mut franchise_descriptions: HashMap<i64, (Option<i32>, String)> = HashMap::new();
    // Products without franchise get their own entry
    let mut standalone: Vec<SeriesCandidate> = Vec::new();

    for (i, item) in items.iter().enumerate() {
        let product = match item.get("product") {
            Some(p) => p,
            None => continue,
        };

        let id = product
            .get("id")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let title = product
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let rating = product
            .get("rating")
            .and_then(|r| r.as_f64())
            .filter(|&r| r > 0.0);

        let franchises = product
            .get("franchises")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default();

        let authors = extract_product_authors(product);

        let synopsis = product
            .get("synopsis")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let vol_num = parsers::extract_metadata_volume(title);

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

        let url_path = product
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let external_url = if url_path.is_empty() {
            None
        } else {
            Some(format!("https://www.senscritique.com{url_path}"))
        };

        let confidence = 1.0 - (i as f32 / 20.0).clamp(0.0, 0.9);

        if let Some(franchise) = franchises.first() {
            let fid = franchise
                .get("id")
                .and_then(|v| v.as_i64())
                .unwrap_or_default();
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
                    "rating_scale": 10.0_f64,
                    "description": synopsis,
                }),
            };

            // Keep the first occurrence per franchise (most relevant from search order)
            franchise_map.entry(fid).or_insert((candidate, rating));

            // Track description from the lowest volume number (tome 1 preferred)
            if let Some(syn) = synopsis.clone() {
                match franchise_descriptions.get(&fid) {
                    None => {
                        franchise_descriptions.insert(fid, (vol_num, syn));
                    }
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
                description: synopsis.clone(),
                publishers: vec![],
                start_year: year,
                total_volumes: None,
                cover_url,
                external_url,
                confidence,
                metadata_json: serde_json::json!({
                    "product_id": id,
                    "rating": rating,
                    "rating_scale": 10.0_f64,
                    "description": synopsis,
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
            let edition_candidates = fetch_franchise_editions(
                &client,
                *fid,
                base_candidate,
                &franchise_descriptions,
                &latest_dates,
                query,
            )
            .await;
            results.extend(edition_candidates);
        }
        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.extend(standalone);
        Ok(results)
    } else {
        // Batch mode: return one candidate per franchise (no extra API calls)
        let mut results: Vec<SeriesCandidate> = franchise_map
            .into_iter()
            .map(|(fid, (mut c, _))| {
                if c.description.is_none() {
                    if let Some((_, desc)) = franchise_descriptions.remove(&fid) {
                        c.description = Some(desc.clone());
                        c.metadata_json["description"] = serde_json::json!(desc);
                    }
                }
                if let Some(date_str) = latest_dates.get(&fid) {
                    let status = infer_status_from_date(date_str);
                    c.metadata_json["status"] = serde_json::json!(status);
                }
                c
            })
            .collect();
        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
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
                items {{ id title url dateRelease medias {{ picture }} authors {{ name }} pencillers {{ name }} synopsis }}
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

    let status = latest_dates
        .get(&franchise_id)
        .map(|d| infer_status_from_date(d));

    editions
        .into_iter()
        .map(|(edition_name, products)| {
            let volume_count = products.len() as i32;

            // Pick cover from tome 1 (earliest volume)
            let cover_url = products
                .iter()
                .filter_map(|p| {
                    let vol = parsers::extract_metadata_volume(
                        p.get("title").and_then(|v| v.as_str()).unwrap_or_default(),
                    );
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
                    let vol = parsers::extract_metadata_volume(title).unwrap_or(i32::MAX);
                    let syn = p
                        .get("synopsis")
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

            let authors = first_volume_product(&products)
                .map(extract_product_authors)
                .unwrap_or_else(|| base_candidate.authors.clone());

            let mut metadata = base_candidate.metadata_json.clone();
            if let Some(ref s) = status {
                metadata["status"] = serde_json::json!(s);
            }
            metadata["edition"] = serde_json::json!(edition_name);
            metadata["description"] = serde_json::json!(description);

            // Confidence based on name similarity with search query
            let similarity = name_similarity(search_query, &edition_name);
            // Bonus for editions with more volumes (main edition likely more relevant)
            let volume_bonus = (volume_count as f32 / 100.0).min(0.1);
            let confidence = (similarity + volume_bonus).min(1.0);

            SeriesCandidate {
                external_id: format!(
                    "franchise:{}:edition:{}",
                    franchise_id,
                    encode_edition(&edition_name)
                ),
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
async fn get_series_impl(external_id: &str) -> Result<SeriesCandidate, String> {
    let books = get_series_books_impl(external_id).await?;
    let first = books
        .iter()
        .min_by_key(|book| book.volume_number.unwrap_or(i32::MAX))
        .ok_or_else(|| format!("SensCritique series {external_id} has no products"))?;
    let start_year = books
        .iter()
        .filter_map(|book| book.publish_date.as_deref())
        .filter_map(|date| date.get(..4))
        .filter_map(|year| year.parse().ok())
        .min();
    let description = first.summary.clone();
    let mut metadata_json = serde_json::json!({});
    if let Some(description) = &description {
        metadata_json["description"] = serde_json::json!(description);
    }
    Ok(SeriesCandidate {
        external_id: external_id.to_string(),
        title: first.title.clone(),
        authors: first.authors.clone(),
        description,
        publishers: vec![],
        start_year,
        total_volumes: Some(books.len() as i32),
        cover_url: first.cover_url.clone(),
        external_url: None,
        confidence: 1.0,
        metadata_json,
    })
}

async fn get_series_books_impl(external_id: &str) -> Result<Vec<BookCandidate>, String> {
    let client = build_client()?;

    if let Some(rest) = external_id.strip_prefix("franchise:") {
        let (fid_str, edition_filter) =
            if let Some((fid_part, edition_part)) = rest.split_once(":edition:") {
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
    } else if external_id.chars().all(|c| c.is_ascii_digit()) {
        // Legacy bare numeric ID — treat as franchise
        let fid: i64 = external_id
            .parse()
            .map_err(|_| format!("invalid legacy franchise id: {external_id}"))?;
        fetch_franchise_books(&client, fid, None).await
    } else {
        Err(format!("unrecognized external_id format: {external_id}"))
    }
}

/// Fetch all books in a franchise via groupProducts, optionally filtered by edition.
async fn fetch_franchise_books(
    client: &reqwest::Client,
    franchise_id: i64,
    edition_filter: Option<&str>,
) -> Result<Vec<BookCandidate>, String> {
    let gql = serde_json::json!({
        "query": format!(
            r#"{{ groupProducts(franchiseId: {franchise_id}, universe: "comicBook", limit: 200, offset: 0) {{
                items {{
                    id title url dateRelease rating
                    medias {{ picture }}
                    authors {{ name }}
                    pencillers {{ name }}
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
            let id = product
                .get("id")
                .and_then(|v| v.as_i64())
                .unwrap_or_default();
            let title = product
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if title.is_empty() {
                return None;
            }

            let volume_number = parsers::extract_metadata_volume(&title);

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

            let authors = extract_product_authors(product);

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
async fn fetch_single_book(
    client: &reqwest::Client,
    product_id: i64,
) -> Result<Vec<BookCandidate>, String> {
    let gql = serde_json::json!({
        "query": format!(
            r#"{{ product(id: {product_id}) {{
                id title url dateRelease rating
                medias {{ picture }}
                authors {{ name }}
                pencillers {{ name }}
                synopsis
            }} }}"#
        ),
    });

    let data = graphql_request(client, &gql).await?;

    let product = data
        .pointer("/data/product")
        .ok_or("SensCritique: product not found")?;

    let id = product
        .get("id")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
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

    let authors = extract_product_authors(product);

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
        "query": "{ poll(id: POLL_ID, limit: LIMIT, offset: 0) { products { id title url medias { picture } authors { name } dateRelease rating synopsis franchises { id label } } } }"
            .replace("POLL_ID", &POLL_ID_MANGA.to_string())
            .replace("LIMIT", &limit.to_string()),
    });

    let client = build_client()?;
    let data = graphql_request(&client, &query).await?;

    let items = data
        .pointer("/data/poll/products")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing poll products in GraphQL response")?;

    // Tops already return series-level products, but use franchise grouping
    // to get franchise:{id} as external_id for proper metadata linking
    Ok(group_products_by_franchise(items, limit))
}

/// Fetch the SensCritique "top 100 BD" list via GraphQL.
pub async fn fetch_top_bd(limit: usize) -> Result<Vec<SeriesCandidate>, String> {
    let query = serde_json::json!({
        "query": format!(
            "{{ top(universe: \"comicBook\", subtype: TOP_100_OUT_OF_TOP_10, limit: {limit}, offset: 0) {{ id title url medias {{ picture }} authors {{ name }} dateRelease rating synopsis franchises {{ id label }} }} }}"
        ),
    });

    let client = build_client()?;
    let data = graphql_request(&client, &query).await?;

    let items = data
        .pointer("/data/top")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing top in GraphQL response")?;

    Ok(group_products_by_franchise(items, limit))
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

    let period_args = if period == "ALL_TIME" {
        "byPeriod: false".to_string()
    } else {
        format!("byPeriod: true, period: {period}")
    };
    let query = serde_json::json!({
        "query": format!(
            "{{ productsByRelease(universe: \"{universe}\", limit: {fetch_limit}, offset: 0, sortBy: {sort_by}, {period_args}) {{ items {{ id title url category medias {{ picture }} authors {{ name }} dateRelease rating synopsis franchises {{ id label }} }} }} }}"
        ),
    });

    let client = build_client()?;
    let data = graphql_request(&client, &query).await?;

    let items = data
        .pointer("/data/productsByRelease/items")
        .and_then(|v| v.as_array())
        .ok_or("SensCritique: missing productsByRelease items in GraphQL response")?;

    // Group products by franchise to show series instead of individual tomes
    let mut candidates = group_products_by_franchise(items, fetch_limit);

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

/// Group products by franchise, keeping the first product's metadata per franchise.
/// Products without a franchise are kept as standalone entries.
fn group_products_by_franchise(items: &[serde_json::Value], limit: usize) -> Vec<SeriesCandidate> {
    let mut seen_franchises: HashMap<i64, usize> = HashMap::new();
    let mut candidates: Vec<SeriesCandidate> = Vec::new();

    for (i, product) in items.iter().take(limit * 2).enumerate() {
        let id = product
            .get("id")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let title = product
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if title.is_empty() {
            continue;
        }

        let franchise = product
            .get("franchises")
            .and_then(|f| f.as_array())
            .and_then(|arr| arr.first());

        let (display_title, external_id) = if let Some(f) = franchise {
            let fid = f.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
            let label = f
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or(&title)
                .to_string();

            // Skip if we already have this franchise
            if seen_franchises.contains_key(&fid) {
                continue;
            }
            seen_franchises.insert(fid, candidates.len());

            (label, format!("franchise:{fid}"))
        } else {
            (title.clone(), id.to_string())
        };

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

        candidates.push(SeriesCandidate {
            external_id,
            title: display_title,
            authors,
            description: description.clone(),
            publishers: vec![],
            start_year: year,
            total_volumes: None,
            cover_url,
            external_url,
            confidence,
            metadata_json: serde_json::json!({
                "source": "senscritique",
                "rating": rating,
                "rating_scale": 10.0_f64,
                "category": category,
                "description": description,
            }),
        });
    }

    candidates
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
/// If the latest volume was released within the last 18 months → "ongoing",
/// otherwise "ended". An unparseable date yields "unknown".
fn infer_status_from_date(date_str: &str) -> &'static str {
    use chrono::{NaiveDate, Utc};
    let cutoff = Utc::now().date_naive() - chrono::Duration::days(18 * 30);
    match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) if date > cutoff => "ongoing",
        Ok(_) => "ended",
        Err(_) => "unknown",
    }
}

// ─── Shared helpers ─────────────────────────────────────────────────────────

async fn graphql_request(
    client: &reqwest::Client,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    graphql_request_url(client, GRAPHQL_URL, body).await
}

pub const RATE_LIMITED_ERROR: &str = "SensCritique GraphQL: rate limited (429)";

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
            // Don't retry on 429 — back off immediately so the caller can circuit-break
            return Err(RATE_LIMITED_ERROR.to_string());
        }

        if !resp.status().is_success() {
            if attempt < max_retries {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                delay_ms *= 2;
                continue;
            }
            return Err(format!(
                "SensCritique GraphQL returned HTTP {}",
                resp.status()
            ));
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

/// SensCritique exposes writers and pencillers as separate fields.
fn extract_product_authors(product: &serde_json::Value) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    extract_names(product, "authors")
        .into_iter()
        .chain(extract_names(product, "pencillers"))
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

/// Return the lowest-numbered volume from an edition's products.
fn first_volume_product<'a>(products: &[&'a serde_json::Value]) -> Option<&'a serde_json::Value> {
    products
        .iter()
        .filter_map(|product| {
            let title = product
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            parsers::extract_metadata_volume(title).map(|volume| (volume, *product))
        })
        .min_by_key(|(volume, _)| *volume)
        .map(|(_, product)| product)
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
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
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
fn group_products_by_edition(
    items: &[serde_json::Value],
) -> Vec<(String, Vec<&serde_json::Value>)> {
    let mut groups: HashMap<String, Vec<&serde_json::Value>> = HashMap::new();

    for item in items {
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if let Some(edition) = extract_edition_name(title) {
            groups.entry(edition).or_default().push(item);
        }
    }

    let mut result: Vec<_> = groups.into_iter().collect();
    // Sort by volume count descending (main edition first)
    result.sort_by_key(|entry| std::cmp::Reverse(entry.1.len()));
    result
}

#[cfg(test)]
#[path = "tests/senscritique.rs"]
mod tests;
