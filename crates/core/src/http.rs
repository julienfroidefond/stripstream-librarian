//! Shared helpers to build `reqwest` clients with consistent defaults.
//!
//! Outbound HTTP calls (metadata providers, scrapers, integrations, downloads)
//! all need a `reqwest::Client`. Building one inline in every call site duplicated
//! the same `builder().timeout(..).build()` boilerplate and drifted on user-agent
//! and timeout values. These helpers centralize construction so the defaults stay
//! aligned; callers that need extra knobs (redirect policy, connect timeout) keep
//! using [`reqwest::Client::builder`] directly.

use std::time::Duration;

/// Default user-agent advertised by outbound metadata and scraping requests.
pub const USER_AGENT: &str = "StripstreamLibrarian/1.0 (metadata; contact administrator)";

/// Build a client with a request timeout and no explicit user-agent.
pub fn build_http_client(timeout: Duration) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder().timeout(timeout).build()
}

/// Build a client with a request timeout and the shared [`USER_AGENT`].
pub fn build_http_client_with_agent(timeout: Duration) -> reqwest::Result<reqwest::Client> {
    build_http_client_with_custom_agent(timeout, USER_AGENT)
}

/// Build a client with a request timeout and a caller-supplied user-agent.
///
/// Used by scrapers that must spoof a browser (e.g. SensCritique) rather than
/// advertise the shared [`USER_AGENT`].
pub fn build_http_client_with_custom_agent(
    timeout: Duration,
    user_agent: &str,
) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(user_agent)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_client_without_agent() {
        build_http_client(Duration::from_secs(15)).expect("client builds");
    }

    #[test]
    fn builds_client_with_shared_agent() {
        build_http_client_with_agent(Duration::from_secs(20)).expect("client builds");
    }

    #[test]
    fn builds_client_with_custom_agent() {
        build_http_client_with_custom_agent(Duration::from_secs(5), "Custom/1.0")
            .expect("client builds");
    }
}
