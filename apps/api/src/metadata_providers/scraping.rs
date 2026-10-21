//! Shared helpers for HTML-scraping metadata providers (BDTheque, BDphile).
//!
//! Both providers fetch pages with the same client settings and post-process the
//! DOM identically (text collapsing, URL absolutization, labeled error messages),
//! so those helpers live here instead of being duplicated per provider.

use std::time::Duration;

use scraper::ElementRef;

/// How many times a transient failure is attempted before giving up.
const MAX_ATTEMPTS: u32 = 3;
/// Base delay between attempts; multiplied by the attempt number (linear backoff).
const RETRY_DELAY: Duration = Duration::from_millis(400);

/// Statuses worth retrying: the site is temporarily overloaded or throttling.
fn is_transient(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status.is_server_error()
}

/// Build an HTTP client with the shared user-agent and a request timeout.
///
/// `provider` is only used to label client-construction errors.
pub(crate) fn client(timeout_secs: u64, provider: &str) -> Result<reqwest::Client, String> {
    stripstream_core::http::build_http_client_with_agent(Duration::from_secs(timeout_secs))
        .map_err(|e| format!("failed to build {provider} client: {e}"))
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

/// GET `url` and return the response body as text.
///
/// Non-2xx responses and transport failures are reported with a `provider`-labelled
/// message so scraper errors stay attributable.
pub(crate) async fn get_html(
    client: &reqwest::Client,
    url: &str,
    provider: &str,
) -> Result<String, String> {
    let mut attempt = 1;
    loop {
        let response = match client.get(url).send().await {
            Ok(response) => response,
            Err(e) if attempt < MAX_ATTEMPTS && (e.is_timeout() || e.is_connect()) => {
                tokio::time::sleep(RETRY_DELAY * attempt).await;
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
        // them; a short retry avoids reporting a whole series as failed.
        if attempt < MAX_ATTEMPTS && is_transient(status) {
            tokio::time::sleep(RETRY_DELAY * attempt).await;
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
        let body = get_html(&client, &server.uri(), "Test").await.unwrap();
        assert_eq!(body, "ok");
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
