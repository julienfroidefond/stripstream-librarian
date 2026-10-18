//! Render a failed outbound HTTP call without leaking the request URL.
//!
//! `reqwest::Error`'s own `Display` embeds the request URL, and for some
//! backends (AllDebrid's signed file links, provider links carrying a session
//! token) that URL is a short-lived credential that must never end up in an
//! `*_downloads.error_message` column served by the API. Every download path
//! therefore persists [`describe_http_failure`] instead of the raw error.

/// One-line description of a failed `reqwest` call that never repeats the
/// request URL.
pub(crate) fn describe_http_failure(err: &reqwest::Error) -> String {
    let kind = if err.is_timeout() {
        "timeout"
    } else if err.is_connect() {
        "connection"
    } else if err.is_body() || err.is_decode() {
        "body"
    } else if err.is_redirect() {
        "redirect"
    } else {
        "request"
    };
    match err.status() {
        Some(status) => format!("{kind} error (HTTP {status})"),
        None => format!("{kind} error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_repeats_the_request_url() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // Nothing listens on port 1, so this always fails at connect time.
        let err = runtime
            .block_on(reqwest::get("http://127.0.0.1:1/unlocked?sig=supersecret"))
            .expect_err("connecting to port 1 must fail");

        let described = describe_http_failure(&err);
        assert!(
            !described.contains("127.0.0.1") && !described.contains("supersecret"),
            "the persisted error must not carry the signed link, got: {described}"
        );
        assert!(described.contains("error"), "got: {described}");
    }
}
