//! HTTP `Range` support for the shared download engine.
//!
//! Direct download providers expose a plain `GET`. When the server honours
//! `Range` requests the transfer can go through
//! [`download_ranged`](super::ranged::download_ranged) with several connections
//! and per-block retries; otherwise the caller falls back to a single stream.
//! [`probe_remote_file`] decides which by reading the response to a one-byte
//! range request.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::header::{CONTENT_RANGE, RANGE};

use super::http_error::describe_http_failure;
use super::ranged::{FetchError, RangeFetcher};

/// What a `Range: bytes=0-0` probe learned about a remote file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteFile {
    /// Total size in bytes, or `0` when the server did not report one.
    pub total_size: i64,
    /// The server answered with `206 Partial Content`, i.e. it honours ranges.
    pub accepts_ranges: bool,
}

/// Ask the server for the first byte of `url` and read the total size back.
///
/// A `206` (with a `Content-Range` header) means byte ranges are supported; a
/// `200` means the header was ignored, so the caller must stream instead. Any
/// other status is a hard failure.
pub async fn probe_remote_file(client: &reqwest::Client, url: &str) -> anyhow::Result<RemoteFile> {
    let response = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("{}", describe_http_failure(&e)))?;

    let status = response.status();
    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let total = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_content_range_total)
            .unwrap_or(0);
        return Ok(RemoteFile {
            total_size: total,
            accepts_ranges: true,
        });
    }
    if status.is_success() {
        return Ok(RemoteFile {
            total_size: response.content_length().map(|l| l as i64).unwrap_or(0),
            accepts_ranges: false,
        });
    }
    anyhow::bail!("DDL provider returned HTTP {status}");
}

/// Parse the total size out of a `Content-Range: bytes 0-0/1234` header.
fn parse_content_range_total(value: &str) -> Option<i64> {
    value
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|total| *total != "*")
        .and_then(|total| total.parse::<i64>().ok())
        .filter(|total| *total > 0)
}

/// [`RangeFetcher`] backed by an HTTP `GET` + `Range`.
pub struct HttpRangeFetcher {
    client: reqwest::Client,
    url: Arc<str>,
}

impl HttpRangeFetcher {
    pub fn new(client: reqwest::Client, url: impl Into<Arc<str>>) -> Self {
        Self {
            client,
            url: url.into(),
        }
    }
}

#[async_trait]
impl RangeFetcher for HttpRangeFetcher {
    async fn fetch(&self, offset: i64, limit: i32) -> Result<Vec<u8>, FetchError> {
        let end = offset + limit as i64 - 1;
        let response = self
            .client
            .get(self.url.as_ref())
            .header(RANGE, format!("bytes={offset}-{end}"))
            .send()
            .await
            .map_err(|e| FetchError::transient(describe_http_failure(&e)))?;

        let status = response.status();
        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            let bytes = response
                .bytes()
                .await
                .map_err(|e| FetchError::transient(describe_http_failure(&e)))?;
            if bytes.len() == limit as usize {
                return Ok(bytes.to_vec());
            }
            return Err(FetchError::transient(format!(
                "short range response: {} of {limit} bytes",
                bytes.len()
            )));
        }
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            return Err(FetchError::fatal(format!(
                "server rejected the byte range (HTTP {status})"
            )));
        }
        if status.is_success() {
            // The server ignored `Range`: retrying this block cannot help, and
            // the caller already decided the file was range-capable.
            return Err(FetchError::fatal(format!(
                "server ignored the byte range (HTTP {status})"
            )));
        }
        // 429/5xx and friends: worth another attempt.
        Err(FetchError::transient(format!("HTTP {status}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::ranged::FetchErrorKind;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client() -> reqwest::Client {
        reqwest::Client::new()
    }

    #[test]
    fn content_range_total_parsing() {
        assert_eq!(parse_content_range_total("bytes 0-0/12345"), Some(12345));
        assert_eq!(parse_content_range_total("bytes 0-0/*"), None);
        assert_eq!(parse_content_range_total("bytes 0-0/0"), None);
        assert_eq!(parse_content_range_total("garbage"), None);
    }

    #[tokio::test]
    async fn probe_detects_range_support() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("range", "bytes=0-0"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 0-0/12345")
                    .set_body_bytes(vec![0u8]),
            )
            .mount(&server)
            .await;

        let probe = probe_remote_file(&client(), &server.uri()).await.unwrap();
        assert_eq!(
            probe,
            RemoteFile {
                total_size: 12345,
                accepts_ranges: true,
            }
        );
    }

    #[tokio::test]
    async fn probe_detects_an_ignored_range() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
            .mount(&server)
            .await;

        let probe = probe_remote_file(&client(), &server.uri()).await.unwrap();
        assert!(!probe.accepts_ranges);
    }

    #[tokio::test]
    async fn probe_reports_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = probe_remote_file(&client(), &server.uri())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("503"), "got: {err}");
    }

    #[tokio::test]
    async fn fetch_returns_an_exact_block() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("range", "bytes=10-19"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(vec![7u8; 10]))
            .mount(&server)
            .await;

        let fetcher = HttpRangeFetcher::new(client(), server.uri());
        let data = fetcher.fetch(10, 10).await.unwrap();
        assert_eq!(data, vec![7u8; 10]);
    }

    #[tokio::test]
    async fn fetch_treats_a_200_as_permanent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1u8; 32]))
            .mount(&server)
            .await;

        let fetcher = HttpRangeFetcher::new(client(), server.uri());
        let err = fetcher.fetch(0, 32).await.unwrap_err();
        assert_eq!(err.kind, FetchErrorKind::Fatal);
    }

    #[tokio::test]
    async fn fetch_retries_server_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let fetcher = HttpRangeFetcher::new(client(), server.uri());
        let err = fetcher.fetch(0, 16).await.unwrap_err();
        assert_eq!(err.kind, FetchErrorKind::Transient);
    }
}
