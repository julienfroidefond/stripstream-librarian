//! Provider execution policy shared by every metadata flow.
//!
//! A [`ProviderRun`] is the state of one job (or one one-shot refresh): it
//! remembers which providers stopped answering and answers "may I call this
//! provider?" for the callers. Adapters describe *why* a call failed with a
//! typed [`ProviderError`]; only a rate limit opens a block, so a transient
//! failure keeps being retried on the next series and a fatal one is just
//! reported. No caller inspects an error message to decide this.

use std::collections::BTreeMap;
use std::future::Future;

use serde::Serialize;
use tracing::warn;
use uuid::Uuid;

use super::provider_label;

// ---------------------------------------------------------------------------
// Provider errors
// ---------------------------------------------------------------------------

/// Why an adapter could not answer.
///
/// The variant is a policy, not a severity: [`ProviderError::RateLimited`] is
/// the only one that opens a [`ProviderRun`] block, [`ProviderError::Transient`]
/// is a network/server hiccup worth retrying later and [`ProviderError::Fatal`]
/// is a request the provider will never answer (bad id, unparsable payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// The provider refused to serve more requests for now (HTTP 429).
    RateLimited(String),
    /// A network or server-side failure worth retrying on a later call.
    Transient(String),
    /// A request the provider will never answer.
    Fatal(String),
}

impl ProviderError {
    /// A network / server-side failure worth retrying later.
    pub fn transient(message: impl Into<String>) -> Self {
        Self::Transient(message.into())
    }

    /// Classify an HTTP failure from its status code, keeping `message`.
    ///
    /// `429` and `5xx` are transient (retrying later may work), anything else
    /// (400, 401, 403, 404, ...) is fatal for this request. A rate limit only
    /// becomes [`ProviderError::RateLimited`] when the adapter knows it must
    /// give up immediately — see SensCritique.
    pub fn http_status(status: u16, message: impl Into<String>) -> Self {
        match status {
            429 | 500..=599 => Self::Transient(message.into()),
            _ => Self::Fatal(message.into()),
        }
    }

    /// The human-readable explanation, without the variant.
    pub fn message(&self) -> &str {
        match self {
            Self::RateLimited(message) | Self::Transient(message) | Self::Fatal(message) => message,
        }
    }

    /// True when the provider refused to serve more requests for now.
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Self::RateLimited(_))
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for ProviderError {}

/// An unclassified adapter message is fatal: it must never open a block.
impl From<String> for ProviderError {
    fn from(message: String) -> Self {
        Self::Fatal(message)
    }
}

impl From<&str> for ProviderError {
    fn from(message: &str) -> Self {
        Self::Fatal(message.to_string())
    }
}

// ---------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------

/// Why a provider is blocked for the rest of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    /// The provider answered HTTP 429.
    RateLimited,
}

/// A provider skipped for the rest of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderBlock {
    /// Canonical provider id (`senscritique`).
    pub provider: String,
    /// Display label (`SensCritique`) used in messages.
    pub label: String,
    pub reason: BlockReason,
    /// The adapter error that opened the block.
    pub message: String,
}

impl ProviderBlock {
    /// The per-series message shown when a lookup was skipped.
    pub fn skip_message(&self) -> String {
        format!("Skipped: {} rate-limited earlier in this job", self.label)
    }
}

// ---------------------------------------------------------------------------
// ProviderRun
// ---------------------------------------------------------------------------

/// The provider execution policy of one metadata job.
///
/// Blocks are keyed by canonical provider id and live as long as the run: the
/// next job starts from a clean [`ProviderRun`]. A block only ever concerns the
/// provider that was rate-limited, so a secondary provider keeps being called
/// after the primary tripped the breaker (#150).
#[derive(Debug, Default)]
pub struct ProviderRun {
    job: Option<Uuid>,
    blocks: BTreeMap<String, ProviderBlock>,
}

impl ProviderRun {
    /// A run that belongs to no job (a one-shot refresh).
    pub fn new() -> Self {
        Self::default()
    }

    /// A run scoped to `job_id`, used in the block logs.
    pub fn for_job(job_id: Uuid) -> Self {
        Self {
            job: Some(job_id),
            blocks: BTreeMap::new(),
        }
    }

    /// True when `provider` was blocked earlier in this run.
    ///
    /// `provider` must be canonical: a discovery source id such as
    /// `senscritique_bd` is a different key from `senscritique`.
    pub fn is_blocked(&self, provider: &str) -> bool {
        self.blocks.contains_key(provider)
    }

    /// The block of `provider`, if it was blocked earlier in this run.
    pub fn block_for(&self, provider: &str) -> Option<&ProviderBlock> {
        self.blocks.get(provider)
    }

    /// Record a failed call, opening a block when it was a rate limit.
    ///
    /// Returns the block when this call opened it (so the caller can log or
    /// report it once) and `None` for a transient/fatal error or a provider
    /// already blocked.
    pub fn record(&mut self, provider: &str, error: &ProviderError) -> Option<&ProviderBlock> {
        if !error.is_rate_limited() || self.blocks.contains_key(provider) {
            return None;
        }
        let block = ProviderBlock {
            provider: provider.to_string(),
            label: provider_label(provider),
            reason: BlockReason::RateLimited,
            message: error.message().to_string(),
        };
        match self.job {
            Some(job) => warn!(
                "[METADATA] job={job} {} rate-limited, skipping its remaining lookups",
                block.label
            ),
            None => warn!(
                "[METADATA] {} rate-limited, skipping its remaining lookups",
                block.label
            ),
        }
        self.blocks.insert(provider.to_string(), block);
        self.blocks.get(provider)
    }

    /// Call `provider` unless it is blocked, and record a rate limit.
    ///
    /// The closure is not called when the provider is already blocked, which is
    /// the whole point of the breaker: a rate-limited provider costs nothing
    /// for the rest of the run.
    pub async fn call<T, F, Fut>(&mut self, provider: &str, call: F) -> ProviderCall<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, ProviderError>>,
    {
        if let Some(block) = self.block_for(provider) {
            return ProviderCall::Skipped(block.clone());
        }
        match call().await {
            Ok(value) => ProviderCall::Answered(Ok(value)),
            Err(error) => {
                self.record(provider, &error);
                ProviderCall::Answered(Err(error))
            }
        }
    }
}

/// What [`ProviderRun::call`] did.
#[derive(Debug, Clone)]
pub enum ProviderCall<T> {
    /// The provider was blocked earlier in the run; nothing was called.
    Skipped(ProviderBlock),
    /// The provider was called and answered (successfully or not).
    Answered(Result<T, ProviderError>),
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn http_status_classifies_429_and_5xx_as_transient() {
        for status in [429, 500, 502, 503, 504] {
            assert_eq!(
                ProviderError::http_status(status, "boom"),
                ProviderError::Transient("boom".to_string()),
                "{status} must be transient"
            );
        }
    }

    #[test]
    fn http_status_classifies_other_codes_as_fatal() {
        for status in [400, 401, 403, 404, 422] {
            assert_eq!(
                ProviderError::http_status(status, "boom"),
                ProviderError::Fatal("boom".to_string()),
                "{status} must be fatal"
            );
        }
    }

    #[test]
    fn unclassified_messages_are_fatal_and_never_rate_limited() {
        let from_string: ProviderError = "SensCritique rate limited (429)".to_string().into();
        assert_eq!(
            from_string,
            ProviderError::Fatal("SensCritique rate limited (429)".to_string())
        );
        assert!(!from_string.is_rate_limited());

        let from_str: ProviderError = "boom".into();
        assert_eq!(from_str, ProviderError::Fatal("boom".to_string()));
    }

    #[test]
    fn transport_errors_are_transient() {
        assert_eq!(
            ProviderError::transient("connection reset"),
            ProviderError::Transient("connection reset".to_string())
        );
    }

    #[test]
    fn display_is_the_bare_message() {
        let error = ProviderError::RateLimited("too many requests".to_string());
        assert_eq!(error.to_string(), "too many requests");
        assert_eq!(error.message(), "too many requests");
    }

    #[test]
    fn rate_limit_opens_a_block_with_the_display_label() {
        let mut run = ProviderRun::new();
        let block = run
            .record("senscritique", &ProviderError::RateLimited("429".into()))
            .expect("a rate limit must open a block")
            .clone();
        assert_eq!(block.provider, "senscritique");
        assert_eq!(block.label, "SensCritique");
        assert_eq!(block.reason, BlockReason::RateLimited);
        assert_eq!(block.message, "429");
        assert!(run.is_blocked("senscritique"));
        assert!(!run.is_blocked("google_books"));
        assert_eq!(run.blocks.len(), 1);
    }

    #[test]
    fn transient_and_fatal_errors_never_block() {
        let mut run = ProviderRun::new();
        assert!(run
            .record("google_books", &ProviderError::transient("timeout"))
            .is_none());
        assert!(run
            .record("google_books", &ProviderError::Fatal("404".into()))
            .is_none());
        assert!(!run.is_blocked("google_books"));
    }

    #[test]
    fn a_block_is_reported_once() {
        let mut run = ProviderRun::new();
        assert!(run
            .record("senscritique", &ProviderError::RateLimited("429".into()))
            .is_some());
        assert!(
            run.record("senscritique", &ProviderError::RateLimited("429".into()))
                .is_none(),
            "a second rate limit must not open a second block"
        );
        assert_eq!(run.blocks.len(), 1);
    }

    #[tokio::test]
    async fn call_skips_a_blocked_provider_without_invoking_it() {
        let mut run = ProviderRun::new();
        run.record("senscritique", &ProviderError::RateLimited("429".into()));

        let called = Cell::new(false);
        let outcome: ProviderCall<u8> = run
            .call("senscritique", || async {
                called.set(true);
                Ok(1)
            })
            .await;

        assert!(!called.get(), "a blocked provider must not be called");
        let ProviderCall::Skipped(block) = outcome else {
            panic!("the call must be skipped");
        };
        assert_eq!(block.label, "SensCritique");
        assert_eq!(
            block.skip_message(),
            "Skipped: SensCritique rate-limited earlier in this job"
        );
    }

    #[tokio::test]
    async fn call_records_a_rate_limit_and_returns_the_error() {
        let mut run = ProviderRun::new();
        let outcome: ProviderCall<u8> = run
            .call("senscritique", || async {
                Err(ProviderError::RateLimited("429".into()))
            })
            .await;

        let ProviderCall::Answered(result) = outcome else {
            panic!("the provider was called");
        };
        assert_eq!(result.unwrap_err().message(), "429");
        assert!(run.is_blocked("senscritique"));
    }

    #[tokio::test]
    async fn call_does_not_block_on_transient_or_fatal_errors() {
        let mut run = ProviderRun::new();
        let _ = run
            .call("google_books", || async {
                Err::<u8, _>(ProviderError::transient("timeout"))
            })
            .await;
        assert!(!run.is_blocked("google_books"));

        let _ = run
            .call("google_books", || async {
                Err::<u8, _>(ProviderError::Fatal("bad id".into()))
            })
            .await;
        assert!(!run.is_blocked("google_books"));
    }

    #[tokio::test]
    async fn a_new_run_recovers_the_blocked_provider() {
        let mut first = ProviderRun::new();
        first.record("senscritique", &ProviderError::RateLimited("429".into()));
        assert!(first.is_blocked("senscritique"));

        let mut next = ProviderRun::new();
        let called = Cell::new(false);
        let outcome: ProviderCall<u8> = next
            .call("senscritique", || async {
                called.set(true);
                Ok(7)
            })
            .await;
        assert!(called.get(), "a fresh run must call the provider again");
        let ProviderCall::Answered(result) = outcome else {
            panic!("a fresh run must call the provider");
        };
        assert_eq!(result.unwrap(), 7);
    }

    #[test]
    fn for_job_keeps_the_job_id() {
        let job = Uuid::new_v4();
        assert_eq!(ProviderRun::for_job(job).job, Some(job));
        assert_eq!(ProviderRun::new().job, None);
    }
}
