//! Shared helpers for HTML-scraping metadata providers (BDTheque, BDphile).
//!
//! Both providers fetch pages with the same client settings and post-process the
//! DOM identically (text collapsing, URL absolutization, labeled error messages),
//! so those helpers live here instead of being duplicated per provider.

use std::sync::OnceLock;
use std::time::Duration;

use futures::StreamExt;
use scraper::ElementRef;
use tracing::warn;

use super::SeriesCandidate;

/// Retry settings for a scraper request.
struct RetryPolicy {
    /// How many times a transient failure is attempted before giving up.
    max_attempts: u32,
    /// Delay before the first retry; doubled on every subsequent attempt.
    base_delay: Duration,
    /// Upper bound for a single retry delay.
    max_delay: Duration,
}

/// Default policy: five attempts, 500 ms doubling up to 5 s.
///
/// Metadata batches hit short BDTheque outages reported as bursts of `500
/// Internal Server Error`. The previous three attempts over ~1.2 s were too
/// short to ride them out, so a whole series was marked failed; this policy
/// keeps retrying for roughly ten seconds before giving up.
const DEFAULT_RETRY: RetryPolicy = RetryPolicy {
    max_attempts: 5,
    base_delay: Duration::from_millis(500),
    max_delay: Duration::from_secs(5),
};

/// `Retry-After` values are clamped so a hostile header cannot stall a job.
const MAX_RETRY_AFTER_SECS: u64 = 10;

/// Shared client for the scraping providers.
static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Statuses worth retrying: the site is temporarily overloaded or throttling.
fn is_transient(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_EARLY
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

/// Delay before the next attempt (1-based).
///
/// `retry_after` carries the parsed `Retry-After` header: when the server asks
/// for a specific pause, honour it instead of the exponential backoff.
fn retry_delay(policy: &RetryPolicy, attempt: u32, retry_after: Option<u64>) -> Duration {
    if let Some(secs) = retry_after {
        return Duration::from_secs(secs.min(MAX_RETRY_AFTER_SECS));
    }
    let factor = 1u32 << attempt.saturating_sub(1).min(5);
    policy
        .base_delay
        .saturating_mul(factor)
        .min(policy.max_delay)
}

/// Parse the `Retry-After` header (delay-seconds form only).
fn retry_after_secs(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Build an HTTP client with the shared user-agent and a request timeout.
///
/// `provider` is only used to label client-construction errors.
pub(crate) fn client(timeout_secs: u64, provider: &str) -> Result<reqwest::Client, String> {
    stripstream_core::http::build_http_client_with_agent(Duration::from_secs(timeout_secs))
        .map_err(|e| format!("failed to build {provider} client: {e}"))
}

/// Shared scraper client, built on first use.
///
/// A provider issues many short requests (a search plus every detail page);
/// reusing one client keeps the connections alive instead of paying a TLS
/// handshake per request, which also spares the origin a burst of new sessions.
/// The timeout is fixed by the first caller, so every provider passes the same
/// value.
pub(crate) fn shared_client(timeout_secs: u64, provider: &str) -> Result<reqwest::Client, String> {
    if let Some(client) = SHARED_CLIENT.get() {
        return Ok(client.clone());
    }
    let built = client(timeout_secs, provider)?;
    Ok(SHARED_CLIENT.get_or_init(|| built).clone())
}

/// Collapse an element's text content into single-spaced, trimmed text.
pub(crate) fn text(el: ElementRef<'_>) -> String {
    el.text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve `href` against `base`, falling back to `href` when either is invalid.
pub(crate) fn absolute(base: &str, href: &str) -> String {
    reqwest::Url::parse(base)
        .ok()
        .and_then(|base| base.join(href).ok())
        .map(|url| url.to_string())
        .unwrap_or_else(|| href.to_string())
}

/// Confidence of a hit against the query that produced it.
///
/// Only an exact (case-insensitive) title match reaches `1.0`, which is the
/// single value that triggers an automatic match during a batch. `max_confidence`
/// caps the result for queries derived from the local name (a variant that
/// dropped part of it must never auto-match).
pub(crate) fn confidence_for(title: &str, query: &str, max_confidence: f32) -> f32 {
    let raw: f32 = if title.eq_ignore_ascii_case(query) {
        1.0
    } else if title.to_lowercase().contains(&query.to_lowercase()) {
        0.7
    } else {
        0.3
    };
    raw.min(max_confidence)
}

/// A search query derived from a local series name.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QueryVariant {
    pub query: String,
    /// Highest confidence a hit found with this query may receive.
    pub max_confidence: f32,
}

/// French articles the scraped sites move to a trailing parenthesis:
/// `Cités obscures (Les)` instead of `Les Cités obscures`.
const TRAILING_ARTICLES: &[&str] = &["les", "le", "la", "l'", "l’"];

/// Confidence ceiling for a query that dropped a subtitle.
///
/// The typeahead only answers when every query word exists in a title, so
/// `Spawn - La Malédiction de Spawn` returns nothing while `Spawn` hits. That
/// stripped form may also be a legitimate sub-series (`Les Légendaires - Saga`
/// vs `Les Légendaires`), hence the cap: such a hit surfaces as a candidate but
/// never auto-matches.
const SUBTITLE_CONFIDENCE_CAP: f32 = 0.7;

/// Collapse every whitespace run into a single space and trim.
fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Move an article back to the front: `Cités obscures (Les)` →
/// `Les Cités obscures`, `Légendaires (Les) - origines` →
/// `Les Légendaires - origines`.
///
/// Only an article closing the name (or followed by a subtitle) is moved; any
/// other parenthesis (`Pilote (édition 1998)`) is part of the title.
fn move_trailing_article(query: &str) -> Option<String> {
    let (open, article, rest) = article_group(query)?;
    let name = query[..open].trim();
    if name.is_empty() {
        return None;
    }
    // Elisions stay glued to the name (`L'` + `Académie` → `L'Académie`).
    let separator = if article.ends_with('\'') || article.ends_with('’') {
        ""
    } else {
        " "
    };
    let mut article = article.to_string();
    if let Some(first) = article.chars().next() {
        if first.is_lowercase() {
            article = first.to_uppercase().collect::<String>() + &article[first.len_utf8()..];
        }
    }
    Some(format!("{article}{separator}{name}{rest}"))
}

/// Locate the last `(article)` group of `query`, with the text that follows it.
fn article_group(query: &str) -> Option<(usize, &str, &str)> {
    let mut found = None;
    for (open, _) in query.match_indices('(') {
        let Some(close) = query[open + 1..].find(')') else {
            continue;
        };
        let article = query[open + 1..open + 1 + close].trim();
        let rest = &query[open + 1 + close + 1..];
        let closes_name = rest.is_empty() || rest.starts_with(" - ") || rest.starts_with(" – ");
        if closes_name && TRAILING_ARTICLES.contains(&article.to_lowercase().as_str()) {
            found = Some((open, article, rest));
        }
    }
    found
}

/// Drop a trailing subtitle: `Troy - L'Expédition d'Alunys` → `Troy`.
fn strip_subtitle(query: &str) -> Option<String> {
    let (head, _) = query
        .split_once(" - ")
        .or_else(|| query.split_once(" – "))?;
    let head = head.trim();
    (!head.is_empty()).then(|| head.to_string())
}

/// Queries to try, in order, when a provider answers "no result".
///
/// Local series names keep the article at the end (`Cités obscures (Les)`) or
/// the album subtitle (`Troy - L'Expédition d'Alunys`), while the scraped
/// catalogues index the plain series title. Each variant is a faithful rewrite
/// of the name, so a hit keeps the intended meaning; the caller tries them in
/// turn and stops at the first non-empty answer.
pub(crate) fn query_variants(query: &str) -> Vec<QueryVariant> {
    let base = collapse_whitespace(query);
    if base.is_empty() {
        return Vec::new();
    }

    let moved = move_trailing_article(&base);
    let moved_and_stripped = moved.as_deref().and_then(strip_subtitle);
    let stripped = strip_subtitle(&base);

    let mut variants: Vec<QueryVariant> = Vec::with_capacity(4);
    let candidates = [
        (Some(base), 1.0),
        (moved, 1.0),
        (moved_and_stripped, 1.0),
        (stripped, SUBTITLE_CONFIDENCE_CAP),
    ];
    for (query, max_confidence) in candidates {
        let Some(query) = query else { continue };
        if variants.iter().any(|known| known.query == query) {
            continue;
        }
        variants.push(QueryVariant {
            query,
            max_confidence,
        });
    }
    variants
}

/// How many search candidates get their detail page fetched.
const MAX_ENRICHED: usize = 8;

/// How many detail pages are fetched at the same time.
///
/// Each candidate is a full series page (~100 KB) and both scraped sites answer
/// bursts with transient 5xx. Fetching eight pages at once per searched series
/// (while a batch already runs back-to-back searches) is what tripped their
/// throttling, so the enrichment walks the candidates in pairs.
const ENRICH_CONCURRENCY: usize = 2;

/// Fetch the detail page of the first [`MAX_ENRICHED`] candidates, preserving
/// their order and limiting concurrency.
///
/// `fetch` receives `(external_id, base_url)` and returns `None` on failure: a
/// missing detail page must never fail the whole search, the candidate simply
/// keeps the fields the search payload already provided.
pub(crate) async fn fetch_candidate_details<F>(
    base: &str,
    candidates: &[SeriesCandidate],
    fetch: F,
) -> Vec<Option<SeriesCandidate>>
where
    F: Fn(String, String) -> futures::future::BoxFuture<'static, Option<SeriesCandidate>>,
{
    let mut requests = Vec::with_capacity(MAX_ENRICHED);
    for candidate in candidates.iter().take(MAX_ENRICHED) {
        requests.push(fetch(candidate.external_id.clone(), base.to_string()));
    }
    futures::stream::iter(requests)
        .buffered(ENRICH_CONCURRENCY)
        .collect()
        .await
}

/// GET `url` and return the response body as text.
///
/// Non-2xx responses and transport failures are reported with a `provider`-labelled
/// message so scraper errors stay attributable.
pub(crate) async fn get_html(
    client: &reqwest::Client,
    url: &str,
    provider: &str,
) -> Result<String, String> {
    get_html_with(client, url, provider, &DEFAULT_RETRY).await
}

/// [`get_html`] with an explicit retry policy (tests use a fast one).
async fn get_html_with(
    client: &reqwest::Client,
    url: &str,
    provider: &str,
    policy: &RetryPolicy,
) -> Result<String, String> {
    let mut attempt = 1;
    loop {
        let response = match client.get(url).send().await {
            Ok(response) => response,
            Err(e) if attempt < policy.max_attempts && (e.is_timeout() || e.is_connect()) => {
                let delay = retry_delay(policy, attempt, None);
                warn!(
                    provider,
                    attempt, url, "retrying after transport error in {delay:?}: {e}"
                );
                tokio::time::sleep(delay).await;
                attempt += 1;
                continue;
            }
            Err(e) => return Err(format!("{provider} request failed: {e}")),
        };

        let status = response.status();
        if status.is_success() {
            return response
                .text()
                .await
                .map_err(|e| format!("failed to read {provider} response: {e}"));
        }
        // Sites commonly answer a transient 5xx / 429 while a batch hammers
        // them; a backed-off retry avoids reporting a whole series as failed.
        if attempt < policy.max_attempts && is_transient(status) {
            let delay = retry_delay(policy, attempt, retry_after_secs(&response));
            warn!(provider, attempt, url, "retrying {status} in {delay:?}");
            tokio::time::sleep(delay).await;
            attempt += 1;
            continue;
        }
        return Err(format!("{provider} returned {status}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_resolves_relative_href() {
        assert_eq!(
            absolute("https://example.com/a/", "b/c"),
            "https://example.com/a/b/c"
        );
    }

    #[test]
    fn absolute_falls_back_to_href_when_base_invalid() {
        assert_eq!(absolute("not a url", "/x"), "/x");
    }

    #[test]
    fn client_builds_with_provider_label() {
        client(20, "Test").expect("client builds");
    }

    #[test]
    fn shared_client_is_built_once() {
        shared_client(20, "Test").expect("client builds");
        assert!(SHARED_CLIENT.get().is_some());
        // Later calls reuse the initialized client (clones share the pool).
        shared_client(20, "Test").expect("client builds");
    }

    #[test]
    fn transient_statuses_cover_server_errors_and_throttling() {
        assert!(is_transient(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_transient(reqwest::StatusCode::BAD_GATEWAY));
        assert!(is_transient(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_transient(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_transient(reqwest::StatusCode::REQUEST_TIMEOUT));
        assert!(is_transient(reqwest::StatusCode::TOO_EARLY));
        assert!(!is_transient(reqwest::StatusCode::BAD_REQUEST));
        assert!(!is_transient(reqwest::StatusCode::FORBIDDEN));
        assert!(!is_transient(reqwest::StatusCode::NOT_FOUND));
    }

    #[test]
    fn retry_delay_doubles_then_caps() {
        let policy = &DEFAULT_RETRY;
        assert_eq!(retry_delay(policy, 1, None), Duration::from_millis(500));
        assert_eq!(retry_delay(policy, 2, None), Duration::from_secs(1));
        assert_eq!(retry_delay(policy, 3, None), Duration::from_secs(2));
        assert_eq!(retry_delay(policy, 4, None), Duration::from_secs(4));
        // 8 s would exceed the cap.
        assert_eq!(retry_delay(policy, 5, None), Duration::from_secs(5));
    }

    #[test]
    fn retry_delay_honours_retry_after_and_clamps_it() {
        let policy = &DEFAULT_RETRY;
        assert_eq!(retry_delay(policy, 1, Some(2)), Duration::from_secs(2));
        assert_eq!(
            retry_delay(policy, 1, Some(3600)),
            Duration::from_secs(MAX_RETRY_AFTER_SECS)
        );
    }

    fn candidate(id: &str) -> SeriesCandidate {
        SeriesCandidate {
            external_id: id.to_string(),
            title: id.to_string(),
            authors: vec![],
            description: None,
            publishers: vec![],
            start_year: None,
            total_volumes: None,
            cover_url: None,
            external_url: None,
            confidence: 0.3,
            metadata_json: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn fetch_candidate_details_bounds_concurrency_and_keeps_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let candidates: Vec<_> = (0..MAX_ENRICHED + 4)
            .map(|i| candidate(&format!("s{i}")))
            .collect();
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let details = fetch_candidate_details("https://example.com", &candidates, |id, _base| {
            let in_flight = in_flight.clone();
            let peak = peak.clone();
            Box::pin(async move {
                let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
                // A failing detail page keeps its slot as `None`.
                (id != "s2").then(|| candidate(&id))
            })
        })
        .await;

        // Only the first MAX_ENRICHED candidates are fetched.
        assert_eq!(details.len(), MAX_ENRICHED);
        let peak = peak.load(Ordering::SeqCst);
        assert!(peak >= 2, "expected concurrent fetches, peak was {peak}");
        assert!(
            peak <= ENRICH_CONCURRENCY,
            "concurrency exceeded: {peak} > {ENRICH_CONCURRENCY}"
        );
        // Order is preserved and a failure does not shift the other results.
        assert_eq!(details[0].as_ref().unwrap().external_id, "s0");
        assert!(details[2].is_none());
        assert_eq!(details[3].as_ref().unwrap().external_id, "s3");
        assert_eq!(
            details[MAX_ENRICHED - 1].as_ref().unwrap().external_id,
            format!("s{}", MAX_ENRICHED - 1)
        );
    }

    /// Fast policy so the retry tests do not spend seconds sleeping.
    fn fast_retry(max_attempts: u32) -> RetryPolicy {
        RetryPolicy {
            max_attempts,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        }
    }

    fn variant(query: &str, max_confidence: f32) -> QueryVariant {
        QueryVariant {
            query: query.to_string(),
            max_confidence,
        }
    }

    #[test]
    fn query_variants_keep_a_plain_name_untouched() {
        assert_eq!(query_variants("Astérix"), vec![variant("Astérix", 1.0)]);
        assert_eq!(
            query_variants("  Les  Géants "),
            vec![variant("Les Géants", 1.0)]
        );
        assert!(query_variants("   ").is_empty());
    }

    #[test]
    fn query_variants_move_a_trailing_article_to_the_front() {
        assert_eq!(
            query_variants("Cités obscures (Les)"),
            vec![
                variant("Cités obscures (Les)", 1.0),
                variant("Les Cités obscures", 1.0),
            ]
        );
        // Elisions stay glued to the name.
        assert_eq!(
            query_variants("Académie Clair obscur (L')"),
            vec![
                variant("Académie Clair obscur (L')", 1.0),
                variant("L'Académie Clair obscur", 1.0),
            ]
        );
        // A parenthesis that is not an article is left alone.
        assert_eq!(
            query_variants("Pilote (édition 1998)"),
            vec![variant("Pilote (édition 1998)", 1.0)]
        );
        // An article in the middle of the name is not the trailing article.
        assert_eq!(
            query_variants("Tintin (Les) et Milou"),
            vec![variant("Tintin (Les) et Milou", 1.0)]
        );
    }

    #[test]
    fn query_variants_strip_a_subtitle_but_cap_its_confidence() {
        assert_eq!(
            query_variants("Spawn - La Malédiction de Spawn"),
            vec![
                variant("Spawn - La Malédiction de Spawn", 1.0),
                variant("Spawn", SUBTITLE_CONFIDENCE_CAP),
            ]
        );
        // Article first, then the subtitle: the most faithful rewrite wins.
        assert_eq!(
            query_variants("Légendaires (Les) - origines"),
            vec![
                variant("Légendaires (Les) - origines", 1.0),
                variant("Les Légendaires - origines", 1.0),
                variant("Les Légendaires", 1.0),
                variant("Légendaires (Les)", SUBTITLE_CONFIDENCE_CAP),
            ]
        );
        // ` – ` is the en dash variant of the same separator.
        assert_eq!(
            query_variants("Arale – Le retour"),
            vec![
                variant("Arale – Le retour", 1.0),
                variant("Arale", SUBTITLE_CONFIDENCE_CAP),
            ]
        );
    }

    #[test]
    fn confidence_for_caps_derived_matches() {
        assert_eq!(
            confidence_for("Les Cités obscures", "Les Cités obscures", 1.0),
            1.0
        );
        assert_eq!(
            confidence_for("Les Cités obscures", "les cités obscures", 1.0),
            1.0
        );
        assert_eq!(confidence_for("Les Cités obscures", "Les Cités", 1.0), 0.7);
        assert_eq!(confidence_for("Bilbo", "Spawn", 1.0), 0.3);
        // A capped variant can never reach the auto-match threshold.
        assert_eq!(
            confidence_for("Spawn", "Spawn", SUBTITLE_CONFIDENCE_CAP),
            SUBTITLE_CONFIDENCE_CAP
        );
    }

    #[tokio::test]
    async fn get_html_retries_transient_server_errors() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(5, "Test").expect("client builds");
        let body = get_html_with(&client, &server.uri(), "Test", &fast_retry(5))
            .await
            .unwrap();
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn get_html_gives_up_after_max_attempts() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .expect(3)
            .mount(&server)
            .await;

        let client = client(5, "Test").expect("client builds");
        let error = get_html_with(&client, &server.uri(), "Test", &fast_retry(3))
            .await
            .unwrap_err();
        assert!(error.contains("503"));
    }

    #[tokio::test]
    async fn get_html_does_not_retry_client_errors() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(400))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(5, "Test").expect("client builds");
        let error = get_html(&client, &server.uri(), "Test").await.unwrap_err();
        assert!(error.contains("400"));
    }
}
