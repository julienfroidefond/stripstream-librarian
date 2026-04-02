use super::SeriesCandidate;

const GRAPHQL_URL: &str = "https://apollo.senscritique.com/";
const POLL_ID_MANGA: i64 = 192836;

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:108.0) Gecko/20100101 Firefox/108.0",
        )
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

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

/// Fetch trending/new releases from SensCritique sorted by popularity.
///
/// `universe` should be `"comicBook"` for all BD/manga/comics.
/// `period` can be `"OUTOFMONTH"`, `"OUTBYWEEK"`, or `"OUTBYYEAR"`.
/// Optionally filter by `category` (e.g. `"Manga"`, `"BD franco-belge"`, `"Comics"`).
pub async fn fetch_trending(
    universe: &str,
    period: &str,
    sort_by: &str,
    category: Option<&str>,
    limit: usize,
) -> Result<Vec<SeriesCandidate>, String> {
    // Fetch more than needed so we can filter by category client-side
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

    // Filter by category if requested
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

            let authors: Vec<String> = product
                .get("authors")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(String::from))
                        .collect()
                })
                .unwrap_or_default();

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
