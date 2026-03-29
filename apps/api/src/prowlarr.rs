use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;

use crate::{error::ApiError, state::AppState};

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
pub struct MissingVolumeInput {
    pub volume_number: Option<i32>,
    #[allow(dead_code)]
    pub title: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct ProwlarrSearchRequest {
    pub series_name: String,
    pub volume_number: Option<i32>,
    pub custom_query: Option<String>,
    pub missing_volumes: Option<Vec<MissingVolumeInput>>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrRawRelease {
    pub guid: String,
    pub title: String,
    pub size: i64,
    pub download_url: Option<String>,
    pub indexer: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub publish_date: Option<String>,
    pub protocol: Option<String>,
    pub info_url: Option<String>,
    pub categories: Option<Vec<ProwlarrCategory>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrRelease {
    pub guid: String,
    pub title: String,
    pub size: i64,
    pub download_url: Option<String>,
    pub indexer: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub publish_date: Option<String>,
    pub protocol: Option<String>,
    pub info_url: Option<String>,
    pub categories: Option<Vec<ProwlarrCategory>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_missing_volumes: Option<Vec<i32>>,
    /// All volumes extracted from the release title (not just missing ones).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_volumes: Vec<i32>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProwlarrCategory {
    pub id: i32,
    pub name: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ProwlarrSearchResponse {
    pub results: Vec<ProwlarrRelease>,
    pub query: String,
}

#[derive(Serialize, ToSchema)]
pub struct ProwlarrTestResponse {
    pub success: bool,
    pub message: String,
    pub indexer_count: Option<i32>,
}

// ─── Config helper ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ProwlarrConfig {
    url: String,
    api_key: String,
    categories: Option<Vec<i32>>,
}

pub(crate) async fn load_prowlarr_config_internal(
    pool: &sqlx::PgPool,
) -> Result<(String, String, Vec<i32>), ApiError> {
    load_prowlarr_config(pool).await
}

pub(crate) async fn check_prowlarr_configured(pool: &sqlx::PgPool) -> Result<(), ApiError> {
    load_prowlarr_config(pool).await.map(|_| ())
}

pub(crate) fn extract_volumes_from_title_pub(title: &str) -> Vec<i32> {
    extract_volumes_from_title(title)
}

/// Returns true if the title indicates a complete/integral edition
/// (e.g., "intégrale", "complet", "complete", "integral").
pub(crate) fn is_integral_release(title: &str) -> bool {
    let lower = title.to_lowercase();
    // Strip accents for matching: "intégrale" → "integrale"
    let normalized = lower
        .replace('é', "e")
        .replace('è', "e");
    let keywords = ["integrale", "integral", "complet", "complete", "l'integrale"];
    keywords.iter().any(|kw| {
        // Match as whole word: check boundaries
        normalized.split(|c: char| !c.is_alphanumeric() && c != '\'')
            .any(|word| word == *kw)
    })
}

async fn load_prowlarr_config(
    pool: &sqlx::PgPool,
) -> Result<(String, String, Vec<i32>), ApiError> {
    let row = sqlx::query("SELECT value FROM app_settings WHERE key = 'prowlarr'")
        .fetch_optional(pool)
        .await?;

    let row = row.ok_or_else(|| ApiError::bad_request("Prowlarr is not configured"))?;
    let value: serde_json::Value = row.get("value");
    let config: ProwlarrConfig = serde_json::from_value(value)
        .map_err(|e| ApiError::internal(format!("invalid prowlarr config: {e}")))?;

    if config.url.is_empty() || config.api_key.is_empty() {
        return Err(ApiError::bad_request(
            "Prowlarr URL and API key must be configured in settings",
        ));
    }

    let url = config.url.trim_end_matches('/').to_string();
    let categories = config.categories.unwrap_or_else(|| vec![7030, 7020]);

    Ok((url, config.api_key, categories))
}

// ─── Volume matching ─────────────────────────────────────────────────────────

/// Extract volume numbers from a release title.
///
/// Handles individual volumes (T01, Tome 01, Vol. 01, v01, #01) and also
/// **range packs** like `T01.T15`, `[T001.T104]`, `T01-T15`, `Tome 01 à Tome 15`
/// — the range is expanded so every volume in [start..=end] is returned.
fn extract_volumes_from_title(title: &str) -> Vec<i32> {
    let lower = title.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut volumes = Vec::new();

    // Pass 1 — range expansion: PREFIX NUMBER (SEP) PREFIX NUMBER
    // Separator: '.' | '-' | 'à'
    let mut i = 0;
    while i < chars.len() {
        if let Some((n1, after1)) = read_vol_prefix_number(&chars, i) {
            let mut j = after1;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            let after_sep = if j < chars.len() && (chars[j] == '.' || chars[j] == '-') {
                Some(j + 1)
            } else if j < chars.len() && chars[j] == '\u{00e0}' {
                // 'à' (U+00E0) — French "à" as in "Tome 01 à Tome 15"
                Some(j + 1)
            } else {
                None
            };

            if let Some(sep_end) = after_sep {
                let mut k = sep_end;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                // Try prefixed number first (T17-T23), then bare number (T17-23)
                let n2_result = read_vol_prefix_number(&chars, k)
                    .or_else(|| read_bare_number(&chars, k));
                if let Some((n2, _)) = n2_result {
                    if n1 < n2 && n2 - n1 <= 500 {
                        for v in n1..=n2 {
                            if !volumes.contains(&v) {
                                volumes.push(v);
                            }
                        }
                        i = after1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    // Pass 2 — individual volumes not already captured by range expansion
    // Note: work entirely with char indices (not byte offsets) to avoid
    // mismatches when the title contains multi-byte UTF-8 characters.
    let prefixes: &[(&[char], bool)] = &[
        (&['t', 'o', 'm', 'e'], false),
        (&['v', 'o', 'l', '.'], false),
        (&['v', 'o', 'l', ' '], false),
        (&['t'], true),
        (&['v'], true),
        (&['#'], false),
    ];
    let len = chars.len();

    for &(prefix, needs_boundary) in prefixes {
        let plen = prefix.len();
        let mut ci = 0usize;
        while ci + plen <= len {
            if chars[ci..ci + plen] != *prefix {
                ci += 1;
                continue;
            }

            // For single-char prefixes (t, v), ensure it's at a word boundary
            if needs_boundary && ci > 0 && chars[ci - 1].is_alphanumeric() {
                ci += plen;
                continue;
            }

            // Skip "v" inside brackets like [V2] — that's a version, not a volume
            if needs_boundary && ci > 0 && chars[ci - 1] == '[' {
                ci += plen;
                continue;
            }

            // Skip optional spaces, dots, or '#' after prefix
            let mut i = ci + plen;
            while i < len && (chars[i] == ' ' || chars[i] == '.' || chars[i] == '#') {
                i += 1;
            }

            // Read digits
            let digit_start = i;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }

            if i > digit_start {
                let num_str: String = chars[digit_start..i].iter().collect();
                if let Ok(num) = num_str.parse::<i32>() {
                    if !volumes.contains(&num) {
                        volumes.push(num);
                    }
                }
            }

            ci += plen;
        }
    }

    // Pass 3 — bare number patterns (only if passes 1 & 2 found nothing)
    // Handles:
    //   "Les Géants - 07 - Moon.cbz"  →  7
    //   "06. yatho.cbz"               →  6
    if volumes.is_empty() {
        // Pattern A: " - NN - " or " - NN." (number between dash separators)
        let dash_num_re = |chars: &[char]| -> Vec<i32> {
            let mut found = Vec::new();
            let mut i = 0;
            while i + 4 < chars.len() {
                // Look for " - "
                if chars[i] == ' ' && chars[i + 1] == '-' && chars[i + 2] == ' ' {
                    let mut j = i + 3;
                    // Skip leading spaces
                    while j < chars.len() && chars[j] == ' ' {
                        j += 1;
                    }
                    let digit_start = j;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > digit_start {
                        // Ensure followed by " - ", ".", or end-ish (space + non-digit)
                        let valid_end = j >= chars.len()
                            || (j + 2 < chars.len() && chars[j] == ' ' && chars[j + 1] == '-' && chars[j + 2] == ' ')
                            || chars[j] == '.'
                            || (chars[j] == ' ' && (j + 1 >= chars.len() || !chars[j + 1].is_ascii_digit()));
                        if valid_end {
                            let num_str: String = chars[digit_start..j].iter().collect();
                            if let Ok(num) = num_str.parse::<i32>() {
                                if !found.contains(&num) {
                                    found.push(num);
                                }
                            }
                        }
                    }
                }
                i += 1;
            }
            found
        };
        volumes.extend(dash_num_re(&chars));

        // Pattern B: "NN. " or "NN - " at the very start of the string
        if volumes.is_empty() {
            let mut j = 0;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > 0 && j < chars.len() {
                let valid_sep = chars[j] == '.' || chars[j] == ' ';
                if valid_sep {
                    let num_str: String = chars[..j].iter().collect();
                    if let Ok(num) = num_str.parse::<i32>() {
                        volumes.push(num);
                    }
                }
            }
        }
    }

    volumes
}

/// Read a bare number (no prefix) at `pos`. Returns `(number, position_after_last_digit)`.
fn read_bare_number(chars: &[char], pos: usize) -> Option<(i32, usize)> {
    let mut i = pos;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i == pos {
        return None;
    }
    let n: i32 = chars[pos..i].iter().collect::<String>().parse().ok()?;
    Some((n, i))
}

/// Try to read a vol-prefixed number starting at `pos` in the `chars` slice.
/// Returns `(number, position_after_last_digit)` or `None`.
/// Prefixes recognised (longest first to avoid "t" matching "tome"):
/// `tome`, `vol.`, `vol `, `t`, `v`, `#`.
fn read_vol_prefix_number(chars: &[char], pos: usize) -> Option<(i32, usize)> {
    if pos >= chars.len() {
        return None;
    }

    // Build a look-ahead string from `pos` (at most 6 chars is enough for the longest prefix "tome ")
    let suffix: String = chars[pos..].iter().collect();

    const PREFIXES: &[(&str, bool)] = &[
        ("tome", false),
        ("vol.", false),
        ("vol ", false),
        ("t", true),
        ("v", true),
        ("#", false),
    ];

    let mut prefix_char_count = 0usize;
    for (p, needs_boundary) in PREFIXES {
        if suffix.starts_with(p) {
            if *needs_boundary && pos > 0 && chars[pos - 1].is_alphanumeric() {
                continue;
            }
            prefix_char_count = p.chars().count();
            break;
        }
    }

    if prefix_char_count == 0 {
        return None;
    }

    let mut i = pos + prefix_char_count;
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '.') {
        i += 1;
    }

    let digit_start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }

    if i == digit_start {
        return None;
    }

    let n: i32 = chars[digit_start..i]
        .iter()
        .collect::<String>()
        .parse()
        .ok()?;
    Some((n, i))
}

/// Match releases against missing volume numbers.
fn match_missing_volumes(
    releases: Vec<ProwlarrRawRelease>,
    missing: &[MissingVolumeInput],
) -> Vec<ProwlarrRelease> {
    let missing_numbers: Vec<i32> = missing
        .iter()
        .filter_map(|m| m.volume_number)
        .collect();

    releases
        .into_iter()
        .map(|r| {
            let title_volumes = extract_volumes_from_title(&r.title);
            let matched = if missing_numbers.is_empty() {
                None
            } else {
                let matched: Vec<i32> = title_volumes
                    .iter()
                    .copied()
                    .filter(|v| missing_numbers.contains(v))
                    .collect();
                if matched.is_empty() {
                    None
                } else {
                    Some(matched)
                }
            };

            ProwlarrRelease {
                guid: r.guid,
                title: r.title,
                size: r.size,
                download_url: r.download_url,
                indexer: r.indexer,
                seeders: r.seeders,
                leechers: r.leechers,
                publish_date: r.publish_date,
                protocol: r.protocol,
                info_url: r.info_url,
                categories: r.categories,
                matched_missing_volumes: matched,
                all_volumes: title_volumes,
            }
        })
        .collect()
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Search for releases on Prowlarr
#[utoipa::path(
    post,
    path = "/prowlarr/search",
    tag = "prowlarr",
    request_body = ProwlarrSearchRequest,
    responses(
        (status = 200, body = ProwlarrSearchResponse),
        (status = 400, description = "Bad request or Prowlarr not configured"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Prowlarr connection error"),
    ),
    security(("Bearer" = []))
)]
pub async fn search_prowlarr(
    State(state): State<AppState>,
    Json(body): Json<ProwlarrSearchRequest>,
) -> Result<Json<ProwlarrSearchResponse>, ApiError> {
    let (url, api_key, categories) = load_prowlarr_config(&state.pool).await?;

    let query = if let Some(custom) = &body.custom_query {
        custom.clone()
    } else if let Some(vol) = body.volume_number {
        format!("\"{}\" {}", body.series_name, vol)
    } else {
        format!("\"{}\"", body.series_name)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Stripstream-Librarian")
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let mut params: Vec<(&str, String)> = vec![
        ("query", query.clone()),
        ("type", "search".to_string()),
    ];
    for cat in &categories {
        params.push(("categories", cat.to_string()));
    }

    let resp = client
        .get(format!("{url}/api/v1/search"))
        .query(&params)
        .header("X-Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Prowlarr request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(ApiError::internal(format!(
            "Prowlarr returned {status}: {text}"
        )));
    }

    let raw_text = resp
        .text()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to read Prowlarr response: {e}")))?;

    tracing::debug!("Prowlarr raw response length: {} chars", raw_text.len());

    let raw_releases: Vec<ProwlarrRawRelease> = serde_json::from_str(&raw_text)
        .map_err(|e| {
            tracing::error!("Failed to parse Prowlarr response: {e}");
            tracing::error!("Raw response (first 500 chars): {}", &raw_text[..raw_text.len().min(500)]);
            ApiError::internal(format!("Failed to parse Prowlarr response: {e}"))
        })?;

    let results = if let Some(missing) = &body.missing_volumes {
        match_missing_volumes(raw_releases, missing)
    } else {
        raw_releases
            .into_iter()
            .map(|r| {
                let all_volumes = extract_volumes_from_title(&r.title);
                ProwlarrRelease {
                    guid: r.guid,
                    title: r.title,
                    size: r.size,
                    download_url: r.download_url,
                    indexer: r.indexer,
                    seeders: r.seeders,
                    leechers: r.leechers,
                    publish_date: r.publish_date,
                    protocol: r.protocol,
                    info_url: r.info_url,
                    categories: r.categories,
                    matched_missing_volumes: None,
                    all_volumes,
                }
            })
            .collect()
    };

    Ok(Json(ProwlarrSearchResponse { results, query }))
}

/// Test connection to Prowlarr
#[utoipa::path(
    get,
    path = "/prowlarr/test",
    tag = "prowlarr",
    responses(
        (status = 200, body = ProwlarrTestResponse),
        (status = 400, description = "Prowlarr not configured"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn test_prowlarr(
    State(state): State<AppState>,
) -> Result<Json<ProwlarrTestResponse>, ApiError> {
    let (url, api_key, _categories) = load_prowlarr_config(&state.pool).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Stripstream-Librarian")
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build HTTP client: {e}")))?;

    let resp = client
        .get(format!("{url}/api/v1/indexer"))
        .header("X-Api-Key", &api_key)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let indexers: Vec<serde_json::Value> = r.json().await.unwrap_or_default();
            Ok(Json(ProwlarrTestResponse {
                success: true,
                message: format!("Connected successfully ({} indexers)", indexers.len()),
                indexer_count: Some(indexers.len() as i32),
            }))
        }
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            Ok(Json(ProwlarrTestResponse {
                success: false,
                message: format!("Prowlarr returned {status}: {text}"),
                indexer_count: None,
            }))
        }
        Err(e) => Ok(Json(ProwlarrTestResponse {
            success: false,
            message: format!("Connection failed: {e}"),
            indexer_count: None,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::extract_volumes_from_title;

    fn sorted(mut v: Vec<i32>) -> Vec<i32> {
        v.sort_unstable();
        v
    }

    #[test]
    fn individual_volumes() {
        assert_eq!(sorted(extract_volumes_from_title("One Piece T05")), vec![5]);
        assert_eq!(sorted(extract_volumes_from_title("Naruto Tome 12")), vec![12]);
        assert_eq!(sorted(extract_volumes_from_title("Vol.03")), vec![3]);
        assert_eq!(sorted(extract_volumes_from_title("v07")), vec![7]);
    }

    #[test]
    fn range_dot_separator() {
        // T01.T15 → 1..=15
        let v = sorted(extract_volumes_from_title("One Piece T01.T15"));
        assert_eq!(v, (1..=15).collect::<Vec<_>>());
    }

    #[test]
    fn range_dot_with_brackets() {
        // [T001.T104] → 1..=104
        let v = sorted(extract_volumes_from_title("Naruto [T001.T104]"));
        assert_eq!(v.len(), 104);
        assert_eq!(v[0], 1);
        assert_eq!(v[103], 104);
    }

    #[test]
    fn range_dash_separator() {
        // T01-T15
        let v = sorted(extract_volumes_from_title("Dragon Ball T01-T10"));
        assert_eq!(v, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn range_french_a_grave() {
        // Tome 01 à Tome 05
        let v = sorted(extract_volumes_from_title("Astérix Tome 01 à Tome 05"));
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn range_long_prefix() {
        // Tome01.Tome15
        let v = sorted(extract_volumes_from_title("Naruto Tome01.Tome15"));
        assert_eq!(v, (1..=15).collect::<Vec<_>>());
    }

    #[test]
    fn range_dash_bare_end() {
        // T17-23 (no prefix on second number) → 17..=23
        let v = sorted(extract_volumes_from_title(
            "Compressé.Demon.Slayer.en.couleurs.T17-23.CBZ.Team.Chromatique",
        ));
        assert_eq!(v, (17..=23).collect::<Vec<_>>());
    }

    #[test]
    fn no_false_positive_version_string() {
        // v2.0 should NOT be treated as a range
        let v = extract_volumes_from_title("tool v2.0 release");
        assert!(!v.contains(&0) || v.len() == 1); // only v2 at most
    }

    #[test]
    fn tome_hash_with_accented_chars() {
        // Tome #097 with accented characters earlier in the string — the é in
        // "Compressé" shifts byte offsets vs char offsets; this must not break parsing.
        // [V2] is a version tag, not a volume — must NOT extract 2.
        let v = sorted(extract_volumes_from_title(
            "[Compressé] One Piece [Team Chromatique] - Tome #097 - [V2].cbz",
        ));
        assert!(v.contains(&97), "expected 97 in {:?}", v);
        assert!(!v.contains(&2), "[V2] should not be extracted as volume 2: {:?}", v);
    }

    #[test]
    fn version_in_brackets_ignored() {
        // [V1], [V2], [V3] are version tags, not volumes
        let v = extract_volumes_from_title("Naruto T05 [V2].cbz");
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn tome_hash_single_digit() {
        let v = sorted(extract_volumes_from_title(
            "[Compressé] One Piece [Team Chromatique] - Tome #003 (Perfect Edition).cbz",
        ));
        assert!(v.contains(&3), "expected 3 in {:?}", v);
    }

    #[test]
    fn bare_number_between_dashes() {
        // "Les Géants - 07 - Moon.cbz" → 7
        let v = extract_volumes_from_title("Les Géants - 07 - Moon.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn bare_number_dash_then_dot() {
        // "Les Géants - 07.cbz" → 7
        let v = extract_volumes_from_title("Les Géants - 07.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn bare_number_at_start_dot() {
        // "06. yatho.cbz" → 6
        let v = extract_volumes_from_title("06. yatho.cbz");
        assert_eq!(v, vec![6]);
    }

    #[test]
    fn bare_number_at_start_dash() {
        // "07 - Moon.cbz" → 7
        let v = extract_volumes_from_title("07 - Moon.cbz");
        assert_eq!(v, vec![7]);
    }

    #[test]
    fn bare_number_no_false_positive_with_prefix() {
        // When a prefix match exists, bare number pass should NOT run
        let v = extract_volumes_from_title("Naruto T05 - some 99 extra.cbz");
        assert_eq!(v, vec![5], "should only find T05, not bare 99");
    }

    use super::is_integral_release;

    #[test]
    fn integral_french_accent() {
        assert!(is_integral_release("One Piece - Intégrale [CBZ]"));
        assert!(is_integral_release("Naruto Integrale FR"));
    }

    #[test]
    fn integral_complet() {
        assert!(is_integral_release("Dragon Ball Complet [PDF]"));
        assert!(is_integral_release("Bleach Complete Edition"));
    }

    #[test]
    fn integral_not_false_positive() {
        assert!(!is_integral_release("One Piece T05"));
        assert!(!is_integral_release("Naruto Tome 12"));
        assert!(!is_integral_release("Les Géants - 07 - Moon.cbz"));
        // "intégr" alone is not enough
        assert!(!is_integral_release("Naruto integration test"));
    }

    #[test]
    fn integral_case_insensitive() {
        assert!(is_integral_release("INTEGRALE"));
        assert!(is_integral_release("COMPLET"));
        assert!(is_integral_release("Intégrale"));
    }
}
