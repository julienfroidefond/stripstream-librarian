use stripstream_core::http::build_http_client;

use super::{compute_confidence, BookCandidate, MetadataProvider, ProviderConfig, SeriesCandidate};

pub struct AniListProvider;

#[async_trait::async_trait]
impl MetadataProvider for AniListProvider {
    fn name(&self) -> &str {
        "anilist"
    }

    async fn search_series(
        &self,
        query: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<SeriesCandidate>, String> {
        search_series_impl(query, config).await
    }

    async fn get_series(
        &self,
        external_id: &str,
        _config: &ProviderConfig,
    ) -> Result<SeriesCandidate, String> {
        get_series_impl(external_id).await
    }

    async fn get_series_books(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<BookCandidate>, String> {
        get_series_books_impl(external_id, config).await
    }
}

const SEARCH_QUERY: &str = r#"
query ($search: String) {
  Page(perPage: 20) {
    media(search: $search, type: MANGA, sort: SEARCH_MATCH) {
      id
      title { romaji english native }
      description(asHtml: false)
      coverImage { large medium }
      startDate { year }
      status
      volumes
      chapters
      staff { edges { node { name { full } } role } }
      siteUrl
      genres
      averageScore
      meanScore
      popularity
    }
  }
}
"#;

const DETAIL_QUERY: &str = r#"
query ($id: Int) {
  Media(id: $id, type: MANGA) {
    id
    title { romaji english native }
    description(asHtml: false)
    coverImage { large medium }
    startDate { year }
    status
    volumes
    chapters
    staff { edges { node { name { full } } role } }
    siteUrl
    genres
    averageScore
    meanScore
    popularity
  }
}
"#;

const ANILIST_GRAPHQL_URL: &str = "https://graphql.anilist.co";

async fn graphql_request_url(
    client: &reqwest::Client,
    url: &str,
    query: &str,
    variables: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "query": query,
            "variables": variables,
        }))
        .send()
        .await
        .map_err(|e| format!("AniList request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("AniList returned {status}: {text}"));
    }

    resp.json()
        .await
        .map_err(|e| format!("Failed to parse AniList response: {e}"))
}

async fn search_series_impl(
    query: &str,
    _config: &ProviderConfig,
) -> Result<Vec<SeriesCandidate>, String> {
    search_series_impl_url(query, ANILIST_GRAPHQL_URL).await
}

async fn search_series_impl_url(query: &str, url: &str) -> Result<Vec<SeriesCandidate>, String> {
    let client = build_http_client(std::time::Duration::from_secs(15))
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let data = graphql_request_url(
        &client,
        url,
        SEARCH_QUERY,
        serde_json::json!({ "search": query }),
    )
    .await?;

    let media = match data
        .get("data")
        .and_then(|d| d.get("Page"))
        .and_then(|p| p.get("media"))
        .and_then(|m| m.as_array())
    {
        Some(media) => media,
        None => return Ok(vec![]),
    };

    let query_lower = query.to_lowercase();

    let mut candidates: Vec<SeriesCandidate> = media
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(|id| id.as_i64())?;
            let title_obj = m.get("title")?;
            let title = title_obj
                .get("english")
                .and_then(|t| t.as_str())
                .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))?
                .to_string();

            let description = m
                .get("description")
                .and_then(|d| d.as_str())
                .map(|d| d.replace("\\n", "\n").trim().to_string())
                .filter(|d| !d.is_empty());

            let cover_url = m
                .get("coverImage")
                .and_then(|ci| ci.get("large").or_else(|| ci.get("medium")))
                .and_then(|u| u.as_str())
                .map(String::from);

            let start_year = m
                .get("startDate")
                .and_then(|sd| sd.get("year"))
                .and_then(|y| y.as_i64())
                .map(|y| y as i32);

            let volumes = m.get("volumes").and_then(|v| v.as_i64()).map(|v| v as i32);

            let chapters = m.get("chapters").and_then(|v| v.as_i64()).map(|v| v as i32);

            let status = m
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("UNKNOWN")
                .to_string();

            let site_url = m.get("siteUrl").and_then(|u| u.as_str()).map(String::from);

            let authors = extract_authors(m);

            let genres = extract_genres(m);

            let average_score = m.get("averageScore").and_then(|v| v.as_f64());
            let mean_score = m.get("meanScore").and_then(|v| v.as_f64());
            let popularity = m.get("popularity").and_then(|v| v.as_i64());
            // Use averageScore, fall back to meanScore
            let rating = average_score.or(mean_score);

            let confidence = compute_confidence(&title, &query_lower);

            let (total_volumes, volume_source) = match volumes {
                Some(v) => (Some(v), "volumes"),
                None => (None, "unknown"),
            };

            Some(SeriesCandidate {
                external_id: id.to_string(),
                title,
                authors,
                description: description.clone(),
                publishers: vec![],
                start_year,
                total_volumes,
                cover_url,
                external_url: site_url,
                confidence,
                metadata_json: serde_json::json!({
                    "status": status,
                    "chapters": chapters,
                    "volumes": volumes,
                    "volume_source": volume_source,
                    "genres": genres,
                    "description": description,
                    "rating": rating,
                    "rating_scale": 100.0_f64,
                    "rating_count": popularity,
                }),
            })
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.truncate(10);
    Ok(candidates)
}

async fn get_series_impl(external_id: &str) -> Result<SeriesCandidate, String> {
    let id: i64 = external_id
        .parse()
        .map_err(|_| "invalid AniList ID".to_string())?;
    let client = build_http_client(std::time::Duration::from_secs(15))
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;
    let data = graphql_request_url(
        &client,
        ANILIST_GRAPHQL_URL,
        DETAIL_QUERY,
        serde_json::json!({ "id": id }),
    )
    .await?;
    let media = data
        .get("data")
        .and_then(|data| data.get("Media"))
        .ok_or_else(|| format!("AniList media {external_id} not found"))?;

    parse_media_to_candidate(media, 1.0)
        .ok_or_else(|| format!("AniList media {external_id} has invalid data"))
}

async fn get_series_books_impl(
    external_id: &str,
    _config: &ProviderConfig,
) -> Result<Vec<BookCandidate>, String> {
    get_series_books_impl_url(external_id, ANILIST_GRAPHQL_URL).await
}

async fn get_series_books_impl_url(
    external_id: &str,
    url: &str,
) -> Result<Vec<BookCandidate>, String> {
    let id: i64 = external_id
        .parse()
        .map_err(|_| "invalid AniList ID".to_string())?;

    let client = build_http_client(std::time::Duration::from_secs(15))
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let data =
        graphql_request_url(&client, url, DETAIL_QUERY, serde_json::json!({ "id": id })).await?;

    let media = match data.get("data").and_then(|d| d.get("Media")) {
        Some(m) => m,
        None => return Ok(vec![]),
    };

    let title_obj = media.get("title").cloned().unwrap_or(serde_json::json!({}));
    let title = title_obj
        .get("english")
        .and_then(|t| t.as_str())
        .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))
        .unwrap_or("")
        .to_string();

    let volumes = media
        .get("volumes")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let cover_url = media
        .get("coverImage")
        .and_then(|ci| ci.get("large").or_else(|| ci.get("medium")))
        .and_then(|u| u.as_str())
        .map(String::from);

    let description = media
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| d.replace("\\n", "\n").trim().to_string());

    let authors = extract_authors(media);

    // AniList doesn't have per-volume data — generate entries from the volume count only
    let mut books = Vec::new();
    if let Some(total) = volumes {
        for vol in 1..=total {
            books.push(BookCandidate {
                external_book_id: format!("{}-vol-{}", external_id, vol),
                title: format!("{} Vol. {}", title, vol),
                volume_number: Some(vol),
                authors: authors.clone(),
                isbn: None,
                summary: if vol == 1 { description.clone() } else { None },
                cover_url: if vol == 1 { cover_url.clone() } else { None },
                page_count: None,
                language: Some("ja".to_string()),
                publish_date: None,
                metadata_json: serde_json::json!({}),
            });
        }
    }

    Ok(books)
}

fn extract_authors(media: &serde_json::Value) -> Vec<String> {
    let mut authors = Vec::new();
    if let Some(edges) = media
        .get("staff")
        .and_then(|s| s.get("edges"))
        .and_then(|e| e.as_array())
    {
        for edge in edges {
            let role = edge.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let role_lower = role.to_lowercase();
            if role_lower.contains("story")
                || role_lower.contains("art")
                || role_lower.contains("original")
            {
                if let Some(name) = edge
                    .get("node")
                    .and_then(|n| n.get("name"))
                    .and_then(|n| n.get("full"))
                    .and_then(|f| f.as_str())
                {
                    if !authors.contains(&name.to_string()) {
                        authors.push(name.to_string());
                    }
                }
            }
        }
    }
    authors
}

// ─── Trending ───────────────────────────────────────────────────────────────

const TRENDING_QUERY: &str = r#"
query ($perPage: Int) {
  Page(perPage: $perPage) {
    media(type: MANGA, sort: TRENDING_DESC) {
      id
      title { romaji english native }
      description(asHtml: false)
      coverImage { large medium }
      startDate { year }
      status
      volumes
      chapters
      staff { edges { node { name { full } } role } }
      siteUrl
      genres
      averageScore
      meanScore
      popularity
    }
  }
}
"#;

/// Fetch trending manga from AniList. Does not go through the MetadataProvider trait.
pub async fn fetch_trending(limit: i32) -> Result<Vec<SeriesCandidate>, String> {
    fetch_trending_url(limit, ANILIST_GRAPHQL_URL).await
}

async fn fetch_trending_url(limit: i32, url: &str) -> Result<Vec<SeriesCandidate>, String> {
    let client = build_http_client(std::time::Duration::from_secs(15))
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let data = graphql_request_url(
        &client,
        url,
        TRENDING_QUERY,
        serde_json::json!({ "perPage": limit.min(50) }),
    )
    .await?;

    let media = match data
        .get("data")
        .and_then(|d| d.get("Page"))
        .and_then(|p| p.get("media"))
        .and_then(|m| m.as_array())
    {
        Some(media) => media,
        None => return Ok(vec![]),
    };

    let candidates: Vec<SeriesCandidate> = media
        .iter()
        .filter_map(|m| parse_media_to_candidate(m, 0.5))
        .collect();

    Ok(candidates)
}

/// Parse a single AniList media JSON object into a SeriesCandidate.
fn parse_media_to_candidate(
    m: &serde_json::Value,
    default_confidence: f32,
) -> Option<SeriesCandidate> {
    let id = m.get("id").and_then(|id| id.as_i64())?;
    let title_obj = m.get("title")?;
    let title = title_obj
        .get("english")
        .and_then(|t| t.as_str())
        .or_else(|| title_obj.get("romaji").and_then(|t| t.as_str()))?
        .to_string();

    let description = m
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| d.replace("\\n", "\n").trim().to_string())
        .filter(|d| !d.is_empty());

    let cover_url = m
        .get("coverImage")
        .and_then(|ci| ci.get("large").or_else(|| ci.get("medium")))
        .and_then(|u| u.as_str())
        .map(String::from);

    let start_year = m
        .get("startDate")
        .and_then(|sd| sd.get("year"))
        .and_then(|y| y.as_i64())
        .map(|y| y as i32);

    let volumes = m.get("volumes").and_then(|v| v.as_i64()).map(|v| v as i32);
    let chapters = m.get("chapters").and_then(|v| v.as_i64()).map(|v| v as i32);

    let status = m
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();

    let site_url = m.get("siteUrl").and_then(|u| u.as_str()).map(String::from);
    let authors = extract_authors(m);
    let genres = extract_genres(m);

    let average_score = m.get("averageScore").and_then(|v| v.as_f64());
    let mean_score = m.get("meanScore").and_then(|v| v.as_f64());
    let popularity = m.get("popularity").and_then(|v| v.as_i64());
    let rating = average_score.or(mean_score);

    let (total_volumes, volume_source) = match volumes {
        Some(v) => (Some(v), "volumes"),
        None => (None, "unknown"),
    };

    Some(SeriesCandidate {
        external_id: id.to_string(),
        title,
        authors,
        description: description.clone(),
        publishers: vec![],
        start_year,
        total_volumes,
        cover_url,
        external_url: site_url,
        confidence: default_confidence,
        metadata_json: serde_json::json!({
            "status": status,
            "chapters": chapters,
            "volumes": volumes,
            "volume_source": volume_source,
            "genres": genres,
            "description": description,
            "rating": rating,
            "rating_scale": 100.0_f64,
            "rating_count": popularity,
        }),
    })
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn extract_genres(media: &serde_json::Value) -> Vec<String> {
    media
        .get("genres")
        .and_then(|g| g.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/anilist.rs"]
mod tests;
