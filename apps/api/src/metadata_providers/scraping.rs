//! Shared helpers for HTML-scraping metadata providers (BDTheque, BDphile).
//!
//! Both providers fetch pages with the same client settings and post-process the
//! DOM identically (text collapsing, URL absolutization, labeled error messages),
//! so those helpers live here instead of being duplicated per provider.

use std::time::Duration;

use scraper::ElementRef;

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
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{provider} request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("{provider} returned {}", response.status()));
    }
    response
        .text()
        .await
        .map_err(|e| format!("failed to read {provider} response: {e}"))
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
}
