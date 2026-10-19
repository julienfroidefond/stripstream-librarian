//! Generic multi-connection ranged download engine.
//!
//! A remote file is split into fixed-size blocks that are fetched concurrently
//! through one or more [`RangeFetcher`]s and written into a [`RangeSink`]. The
//! engine is deliberately protocol- and transport-agnostic so it can be
//! unit-tested with in-memory doubles and driven by the Telegram downloader in
//! production.
//!
//! Design notes:
//!
//! * Every worker dynamically claims the next block index from a shared atomic
//!   counter, so a slow connection never stalls the others.
//! * Each block is retried in place ([`RangedConfig::max_attempts`]) with
//!   exponential backoff, which means a transient error never discards the
//!   blocks already written — the file is only assembled once, unlike a
//!   restart-from-zero retry loop.
//! * A server-imposed cooldown (`FLOOD_WAIT`) does not count as a failed
//!   attempt: the block waits it out, bounded separately by
//!   [`RangedConfig::max_server_cooldown`].
//! * A per-request stall timeout bounds *inactivity*, not the total duration, so
//!   arbitrarily large files can be downloaded.
//! * Progress is reported on a timer through [`ProgressReporter`].

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

/// Whether a failed block fetch is worth retrying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchErrorKind {
    /// Transient failure (network hiccup, flood wait): retry the same block.
    Transient,
    /// Permanent failure (bad file reference, unsupported media): abort.
    Fatal,
}

/// Error returned by a [`RangeFetcher`] for a single block.
#[derive(Debug, Clone)]
pub struct FetchError {
    pub kind: FetchErrorKind,
    pub message: String,
    /// Server-imposed cooldown before retrying (e.g. Telegram `FLOOD_WAIT`).
    pub retry_after: Option<Duration>,
}

impl FetchError {
    /// A permanent error: retrying the same block is pointless.
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            kind: FetchErrorKind::Fatal,
            message: message.into(),
            retry_after: None,
        }
    }

    /// A transient error: the same block is retried after a backoff.
    pub fn transient(message: impl Into<String>) -> Self {
        Self {
            kind: FetchErrorKind::Transient,
            message: message.into(),
            retry_after: None,
        }
    }

    /// A transient error with a server-requested cooldown.
    pub fn retry_in(message: impl Into<String>, retry_after: Duration) -> Self {
        Self {
            kind: FetchErrorKind::Transient,
            message: message.into(),
            retry_after: Some(retry_after),
        }
    }
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FetchError {}

/// Fetches a byte range from a remote resource.
///
/// Implementations should return exactly `limit` bytes, or an error. The final
/// block may legitimately come back shorter when the transport rounds the
/// requested length up and truncates at the end of the file (Telegram's
/// `upload.getFile` requires a 4 KiB-aligned `limit`); the engine accepts that
/// as long as the bytes reach `total_size`. Any other short body is treated as
/// a retryable inconsistency.
#[async_trait]
pub trait RangeFetcher: Send + Sync {
    async fn fetch(&self, offset: i64, limit: i32) -> Result<Vec<u8>, FetchError>;
}

/// Destination for the downloaded bytes.
#[async_trait]
pub trait RangeSink: Send + Sync {
    /// Write `data` starting at `offset`.
    async fn write_at(&self, offset: i64, data: &[u8]) -> std::io::Result<()>;
    /// Flush everything to stable storage.
    async fn finish(&self) -> std::io::Result<()>;
}

/// Receives the total number of bytes successfully written so far.
#[async_trait]
pub trait ProgressReporter: Send + Sync {
    async fn report(&self, bytes_written: i64);
}

/// Tuning knobs for [`download_ranged`].
#[derive(Debug, Clone)]
pub struct RangedConfig {
    /// Size of a single block; the last block may be shorter.
    pub chunk_size: i64,
    /// Concurrent in-flight requests per fetcher.
    pub workers_per_fetcher: usize,
    /// Total attempts per block before giving up.
    pub max_attempts: usize,
    /// Total time a single block may spend honouring server-imposed cooldowns
    /// (Telegram's `FLOOD_WAIT`, for instance) before the transfer is abandoned.
    ///
    /// Such cooldowns do **not** count as failed attempts: a busy account can
    /// legitimately pause a block for minutes without anything being wrong with
    /// the data, and aborting the whole file after five retries would turn a
    /// transient throttle into a failed download.
    pub max_server_cooldown: Duration,
    /// Maximum inactivity before a single request is considered stalled.
    pub stall_timeout: Duration,
    /// How often progress is reported.
    pub progress_interval: Duration,
    /// First backoff delay; doubles on every attempt up to [`Self::max_backoff`].
    pub initial_backoff: Duration,
    /// Upper bound for the exponential backoff.
    pub max_backoff: Duration,
}

impl Default for RangedConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512 * 1024,
            workers_per_fetcher: 2,
            max_attempts: 5,
            max_server_cooldown: Duration::from_secs(30 * 60),
            stall_timeout: Duration::from_secs(180),
            progress_interval: Duration::from_secs(1),
            initial_backoff: Duration::from_secs(2),
            max_backoff: Duration::from_secs(30),
        }
    }
}

impl RangedConfig {
    /// Exponential backoff for the 1-based `attempt` number.
    fn backoff(&self, attempt: usize) -> Duration {
        let exponent = attempt.saturating_sub(1).min(16) as u32;
        self.initial_backoff
            .saturating_mul(2u32.saturating_pow(exponent))
            .min(self.max_backoff)
    }
}

/// Failure of a whole [`download_ranged`] run.
#[derive(Debug)]
pub enum RangedError {
    /// No fetcher was supplied although the file is not empty.
    NoFetchers,
    /// A block failed permanently (or exhausted its attempts).
    Fetch { offset: i64, source: FetchError },
    /// A block did not arrive within the stall timeout.
    Stalled { offset: i64 },
    /// A block body did not have the expected length.
    Incomplete {
        offset: i64,
        expected: i64,
        received: i64,
    },
    /// The total number of written bytes does not match the expected size.
    SizeMismatch { expected: i64, written: i64 },
    /// Writing to the sink failed.
    Io(std::io::Error),
    /// A worker task panicked.
    Worker(String),
}

impl std::fmt::Display for RangedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFetchers => write!(f, "no download connection available"),
            Self::Fetch { offset, source } => {
                write!(f, "failed to fetch block at offset {offset}: {source}")
            }
            Self::Stalled { offset } => {
                write!(f, "download stalled at offset {offset}")
            }
            Self::Incomplete {
                offset,
                expected,
                received,
            } => write!(
                f,
                "incomplete block at offset {offset}: expected {expected} bytes, got {received}"
            ),
            Self::SizeMismatch { expected, written } => write!(
                f,
                "size mismatch after download: expected {expected} bytes, wrote {written}"
            ),
            Self::Io(err) => write!(f, "writing the destination failed: {err}"),
            Self::Worker(err) => write!(f, "download worker failed: {err}"),
        }
    }
}

impl std::error::Error for RangedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Fetch { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Download `total_size` bytes into `sink`, spreading blocks over `fetchers`.
///
/// `total_size` is expected to be known upfront (Telegram documents always are).
/// Returns once every block has been written and the sink flushed.
pub async fn download_ranged(
    fetchers: Vec<Arc<dyn RangeFetcher>>,
    sink: Arc<dyn RangeSink>,
    total_size: i64,
    reporter: Arc<dyn ProgressReporter>,
    config: RangedConfig,
) -> Result<(), RangedError> {
    if total_size <= 0 {
        sink.finish().await.map_err(RangedError::Io)?;
        reporter.report(0).await;
        return Ok(());
    }
    if fetchers.is_empty() {
        return Err(RangedError::NoFetchers);
    }

    let chunk_size = config.chunk_size.max(1);
    let total_blocks = (total_size as u64).div_ceil(chunk_size as u64);
    let next_block = Arc::new(AtomicU64::new(0));
    let written = Arc::new(AtomicI64::new(0));

    let mut set: JoinSet<Result<(), RangedError>> = JoinSet::new();
    for fetcher in fetchers {
        for _ in 0..config.workers_per_fetcher.max(1) {
            let fetcher = fetcher.clone();
            let sink = sink.clone();
            let next_block = next_block.clone();
            let written = written.clone();
            let config = config.clone();
            set.spawn(async move {
                worker(
                    fetcher,
                    sink,
                    next_block,
                    written,
                    total_blocks,
                    total_size,
                    config,
                )
                .await
            });
        }
    }

    // The first tick completes immediately; consume it so nothing is reported
    // before any work has been done.
    let mut ticker = tokio::time::interval(config.progress_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ticker.tick().await;

    let mut failure: Option<RangedError> = None;
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                reporter.report(written.load(Ordering::Relaxed)).await;
            }
            joined = set.join_next() => {
                match joined {
                    None => break,
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(err))) => {
                        failure = Some(err);
                        set.abort_all();
                        break;
                    }
                    Some(Err(join_err)) => {
                        failure = Some(RangedError::Worker(join_err.to_string()));
                        set.abort_all();
                        break;
                    }
                }
            }
        }
    }
    // Drain any aborted worker so nothing keeps running after we return.
    set.shutdown().await;

    if let Some(err) = failure {
        return Err(err);
    }
    drop(set);

    let written = written.load(Ordering::Relaxed);
    if written != total_size {
        return Err(RangedError::SizeMismatch {
            expected: total_size,
            written,
        });
    }

    sink.finish().await.map_err(RangedError::Io)?;
    reporter.report(written).await;
    Ok(())
}

async fn worker(
    fetcher: Arc<dyn RangeFetcher>,
    sink: Arc<dyn RangeSink>,
    next_block: Arc<AtomicU64>,
    written: Arc<AtomicI64>,
    total_blocks: u64,
    total_size: i64,
    config: RangedConfig,
) -> Result<(), RangedError> {
    loop {
        let block = next_block.fetch_add(1, Ordering::Relaxed);
        if block >= total_blocks {
            return Ok(());
        }
        let offset = (block * config.chunk_size as u64) as i64;
        let limit = (total_size - offset).clamp(1, config.chunk_size) as i32;
        let data = fetch_block(&*fetcher, offset, limit, total_size, &config).await?;
        sink.write_at(offset, &data)
            .await
            .map_err(RangedError::Io)?;
        written.fetch_add(data.len() as i64, Ordering::Relaxed);
    }
}

/// Fetch a single block, retrying transient failures and stall timeouts.
async fn fetch_block(
    fetcher: &dyn RangeFetcher,
    offset: i64,
    limit: i32,
    total_size: i64,
    config: &RangedConfig,
) -> Result<Vec<u8>, RangedError> {
    let max_attempts = config.max_attempts.max(1);
    let mut failures = 0usize;
    let mut server_cooldown = Duration::ZERO;
    loop {
        failures += 1;
        let last_attempt = failures >= max_attempts;

        let outcome =
            tokio::time::timeout(config.stall_timeout, fetcher.fetch(offset, limit)).await;
        match outcome {
            Err(_elapsed) => {
                if last_attempt {
                    return Err(RangedError::Stalled { offset });
                }
                tokio::time::sleep(config.backoff(failures)).await;
            }
            Ok(Ok(data)) if data.len() == limit as usize => return Ok(data),
            Ok(Ok(data))
                if data.len() <= limit as usize && offset + data.len() as i64 == total_size =>
            {
                // Transport-truncated final block: the request was rounded up
                // past the end of the file, so the server returned only what is
                // left. This still covers the whole block.
                return Ok(data);
            }
            Ok(Ok(data)) => {
                if last_attempt {
                    return Err(RangedError::Incomplete {
                        offset,
                        expected: limit as i64,
                        received: data.len() as i64,
                    });
                }
                tokio::time::sleep(config.backoff(failures)).await;
            }
            Ok(Err(err)) => match err.kind {
                FetchErrorKind::Fatal => {
                    return Err(RangedError::Fetch {
                        offset,
                        source: err,
                    })
                }
                FetchErrorKind::Transient => {
                    if let Some(delay) = err.retry_after {
                        // A server-requested cooldown (Telegram `FLOOD_WAIT`) is
                        // a pause, not a failed attempt: the block data itself is
                        // still expected. Only the cumulative cooldown budget can
                        // give up, so a short flood never fails the transfer.
                        server_cooldown += delay;
                        if server_cooldown > config.max_server_cooldown {
                            return Err(RangedError::Fetch {
                                offset,
                                source: err,
                            });
                        }
                        failures -= 1;
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    if last_attempt {
                        return Err(RangedError::Fetch {
                            offset,
                            source: err,
                        });
                    }
                    tokio::time::sleep(config.backoff(failures)).await;
                }
            },
        }
    }
}

/// [`RangeSink`] backed by a file: pre-allocated to its final size, then filled
/// in place at arbitrary offsets.
pub struct FileSink {
    file: Mutex<tokio::fs::File>,
}

impl FileSink {
    /// Create (or truncate) `path` and pre-allocate it to `size` bytes.
    pub async fn create(path: &std::path::Path, size: i64) -> std::io::Result<Self> {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .await?;
        file.set_len(size.max(0) as u64).await?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

#[async_trait]
impl RangeSink for FileSink {
    async fn write_at(&self, offset: i64, data: &[u8]) -> std::io::Result<()> {
        let mut file = self.file.lock().await;
        file.seek(std::io::SeekFrom::Start(offset as u64)).await?;
        file.write_all(data).await
    }

    async fn finish(&self) -> std::io::Result<()> {
        let file = self.file.lock().await;
        file.sync_all().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    /// Deterministic byte pattern so the assembled file can be verified
    /// position-by-position.
    fn pattern(offset: i64, len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| (((offset as usize + i) % 251) + 1) as u8)
            .collect()
    }

    #[derive(Default)]
    struct Plan {
        /// block index → remaining transient failures.
        transient: HashMap<u64, usize>,
        /// block index that always fails fatally.
        fatal: Option<u64>,
        /// block indexes that return a body shorter than requested.
        short: HashSet<u64>,
        /// artificial delay before answering.
        delay: Option<Duration>,
        /// When set, transient failures carry a server-requested cooldown
        /// (Telegram `FLOOD_WAIT`) of this duration instead of a plain backoff.
        retry_in: Option<Duration>,
        /// Emulate a transport that rounds requests up to this many bytes
        /// (Telegram requires a 4 KiB-aligned `limit`).
        request_alignment: Option<i64>,
        /// Total file size, so a rounded-up final request is truncated at EOF
        /// like Telegram does.
        total_size: Option<i64>,
    }

    /// Mock fetcher that serves a byte pattern and can inject failures.
    struct MockFetcher {
        id: usize,
        chunk_size: i64,
        plan: StdMutex<Plan>,
        used: Arc<StdMutex<Vec<usize>>>,
        /// shared across every fetcher so the call count is global.
        calls: Arc<AtomicUsize>,
    }

    impl MockFetcher {
        fn new(id: usize, chunk_size: i64, shared_calls: Arc<AtomicUsize>) -> Self {
            Self {
                id,
                chunk_size,
                plan: StdMutex::new(Plan::default()),
                used: Arc::new(StdMutex::new(Vec::new())),
                calls: shared_calls,
            }
        }
    }

    #[async_trait]
    impl RangeFetcher for MockFetcher {
        async fn fetch(&self, offset: i64, limit: i32) -> Result<Vec<u8>, FetchError> {
            // Yield so every worker actually gets scheduled: without it a single
            // task could greedily drain the block counter.
            tokio::task::yield_now().await;
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.used.lock().unwrap().push(self.id);

            let block = offset as u64 / self.chunk_size as u64;
            let (delay, fatal, short, transient, alignment, total_size, retry_in) = {
                let mut plan = self.plan.lock().unwrap();
                let remaining = plan.transient.get_mut(&block).and_then(|r| {
                    if *r > 0 {
                        *r -= 1;
                        Some(())
                    } else {
                        None
                    }
                });
                (
                    plan.delay,
                    plan.fatal == Some(block),
                    plan.short.contains(&block),
                    remaining.is_some(),
                    plan.request_alignment,
                    plan.total_size,
                    plan.retry_in,
                )
            };

            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }
            if fatal {
                return Err(FetchError::fatal("fatal block"));
            }
            if transient {
                return Err(match retry_in {
                    Some(delay) => FetchError::retry_in("flood wait", delay),
                    None => FetchError::transient("transient block"),
                });
            }
            let requested = match alignment {
                Some(alignment) => (limit as i64 + alignment - 1) / alignment * alignment,
                None => limit as i64,
            };
            let available = total_size
                .map(|total| (total - offset).min(requested))
                .unwrap_or(requested);
            let len = if short {
                (limit as usize) / 2
            } else {
                available as usize
            };
            Ok(pattern(offset, len))
        }
    }

    struct MemorySink {
        buf: Mutex<Vec<u8>>,
    }

    impl MemorySink {
        fn new(size: usize) -> Arc<Self> {
            Arc::new(Self {
                buf: Mutex::new(vec![0u8; size]),
            })
        }

        async fn contents(&self) -> Vec<u8> {
            self.buf.lock().await.clone()
        }
    }

    #[async_trait]
    impl RangeSink for MemorySink {
        async fn write_at(&self, offset: i64, data: &[u8]) -> std::io::Result<()> {
            let mut buf = self.buf.lock().await;
            let start = offset as usize;
            buf[start..start + data.len()].copy_from_slice(data);
            Ok(())
        }

        async fn finish(&self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct Recorder {
        reports: Mutex<Vec<i64>>,
    }

    #[async_trait]
    impl ProgressReporter for Recorder {
        async fn report(&self, bytes_written: i64) {
            self.reports.lock().await.push(bytes_written);
        }
    }

    fn fast_config(chunk_size: i64) -> RangedConfig {
        RangedConfig {
            chunk_size,
            workers_per_fetcher: 2,
            max_attempts: 3,
            max_server_cooldown: Duration::from_secs(1),
            stall_timeout: Duration::from_millis(200),
            progress_interval: Duration::from_millis(5),
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(5),
        }
    }

    fn fetchers(
        count: usize,
        chunk_size: i64,
        calls: Arc<AtomicUsize>,
    ) -> (Vec<Arc<dyn RangeFetcher>>, Vec<Arc<MockFetcher>>) {
        let mocks: Vec<_> = (0..count)
            .map(|id| Arc::new(MockFetcher::new(id, chunk_size, calls.clone())))
            .collect();
        let dyns: Vec<Arc<dyn RangeFetcher>> = mocks
            .iter()
            .map(|m| m.clone() as Arc<dyn RangeFetcher>)
            .collect();
        (dyns, mocks)
    }

    #[tokio::test]
    async fn assembles_blocks_with_a_partial_last_block() {
        let chunk = 8;
        let total = chunk * 5 + 3;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, _) = fetchers(3, chunk, calls.clone());
        let reporter = Arc::new(Recorder::default());

        download_ranged(
            fetchers,
            sink.clone(),
            total,
            reporter.clone(),
            fast_config(chunk),
        )
        .await
        .unwrap();

        assert_eq!(sink.contents().await, pattern(0, total as usize));
        let reports = reporter.reports.lock().await.clone();
        assert_eq!(reports.last().copied(), Some(total));
    }

    #[tokio::test]
    async fn accepts_a_transport_truncated_final_block() {
        // Telegram rejects a `limit` that is not a multiple of 4 KiB, so the
        // last (short) block is requested rounded up and comes back truncated.
        // This is exactly what failed in production: the engine treated the
        // shorter final body as an error.
        let chunk = 8; // stands in for 512 KiB
        let total = chunk * 4 + 3;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(3, chunk, calls.clone());
        for mock in &mocks {
            let mut plan = mock.plan.lock().unwrap();
            plan.request_alignment = Some(4);
            plan.total_size = Some(total);
        }

        download_ranged(
            fetchers,
            sink.clone(),
            total,
            Arc::new(Recorder::default()),
            fast_config(chunk),
        )
        .await
        .unwrap();

        assert_eq!(sink.contents().await, pattern(0, total as usize));
    }

    #[tokio::test]
    async fn uses_every_configured_fetcher() {
        let chunk = 4;
        let total = chunk * 200;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(3, chunk, calls.clone());

        download_ranged(
            fetchers,
            sink,
            total,
            Arc::new(Recorder::default()),
            fast_config(chunk),
        )
        .await
        .unwrap();

        let used: HashSet<usize> = mocks
            .iter()
            .flat_map(|m| m.used.lock().unwrap().clone())
            .collect();
        assert_eq!(used, HashSet::from([0, 1, 2]));
    }

    #[tokio::test]
    async fn retries_transient_failures_without_gaps() {
        let chunk = 8;
        let total = chunk * 8;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(3, chunk, calls.clone());
        // Every fetcher knows about the same failing blocks, so whichever one
        // claims a block observes the failure exactly once.
        for mock in &mocks {
            let mut plan = mock.plan.lock().unwrap();
            for block in 0..3u64 {
                plan.transient.insert(block, 1);
            }
        }

        download_ranged(
            fetchers,
            sink.clone(),
            total,
            Arc::new(Recorder::default()),
            fast_config(chunk),
        )
        .await
        .unwrap();

        assert_eq!(sink.contents().await, pattern(0, total as usize));
        // 8 successful blocks + 3 blocks retried once.
        assert_eq!(calls.load(Ordering::Relaxed), 11);
    }

    #[tokio::test]
    async fn server_cooldowns_do_not_consume_block_attempts() {
        let chunk = 8;
        let total = chunk;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(1, chunk, calls.clone());
        {
            let mut plan = mocks[0].plan.lock().unwrap();
            // More floods than `max_attempts`: a FLOOD_WAIT must be waited out,
            // never counted as a failed attempt.
            plan.transient.insert(0, 6);
            plan.retry_in = Some(Duration::from_millis(1));
        }

        let mut config = fast_config(chunk);
        config.max_attempts = 2;

        download_ranged(
            fetchers,
            sink.clone(),
            total,
            Arc::new(Recorder::default()),
            config,
        )
        .await
        .expect("a flood wait must not exhaust the block attempts");

        assert_eq!(sink.contents().await, pattern(0, total as usize));
        assert_eq!(calls.load(Ordering::Relaxed), 7, "6 floods + 1 success");
    }

    #[tokio::test]
    async fn server_cooldown_budget_is_bounded() {
        let chunk = 8;
        let total = chunk;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(1, chunk, calls.clone());
        {
            let mut plan = mocks[0].plan.lock().unwrap();
            plan.transient.insert(0, 100);
            plan.retry_in = Some(Duration::from_millis(5));
        }

        let mut config = fast_config(chunk);
        config.max_server_cooldown = Duration::from_millis(12);

        let err = download_ranged(fetchers, sink, total, Arc::new(Recorder::default()), config)
            .await
            .unwrap_err();

        match err {
            RangedError::Fetch { offset, source } => {
                assert_eq!(offset, 0);
                assert_eq!(source.kind, FetchErrorKind::Transient);
            }
            other => panic!("unexpected error: {other}"),
        }
        // Two 5 ms cooldowns fit in the 12 ms budget, the third exceeds it.
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn fatal_error_aborts_immediately() {
        let chunk = 8;
        let total = chunk * 10;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(2, chunk, calls.clone());
        for mock in &mocks {
            mock.plan.lock().unwrap().fatal = Some(3);
        }

        let err = download_ranged(
            fetchers,
            sink,
            total,
            Arc::new(Recorder::default()),
            fast_config(chunk),
        )
        .await
        .unwrap_err();

        match err {
            RangedError::Fetch { offset, source } => {
                assert_eq!(offset, 24);
                assert_eq!(source.kind, FetchErrorKind::Fatal);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn stall_timeout_is_per_attempt() {
        let chunk = 8;
        let total = chunk * 4;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(1, chunk, calls.clone());
        for mock in &mocks {
            mock.plan.lock().unwrap().delay = Some(Duration::from_millis(200));
        }

        let mut config = fast_config(chunk);
        config.stall_timeout = Duration::from_millis(20);
        config.max_attempts = 2;

        let err = download_ranged(fetchers, sink, total, Arc::new(Recorder::default()), config)
            .await
            .unwrap_err();

        assert!(matches!(err, RangedError::Stalled { .. }), "got {err}");
    }

    #[tokio::test]
    async fn short_bodies_reported_as_incomplete() {
        let chunk = 8;
        let total = chunk * 4;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, mocks) = fetchers(1, chunk, calls.clone());
        for mock in &mocks {
            mock.plan.lock().unwrap().short.insert(0);
        }

        let mut config = fast_config(chunk);
        config.max_attempts = 2;

        let err = download_ranged(fetchers, sink, total, Arc::new(Recorder::default()), config)
            .await
            .unwrap_err();

        match err {
            RangedError::Incomplete {
                offset,
                expected,
                received,
            } => {
                assert_eq!(offset, 0);
                assert_eq!(expected, chunk);
                assert_eq!(received, chunk / 2);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn progress_reports_are_monotonic() {
        let chunk = 4;
        let total = chunk * 50;
        let sink = MemorySink::new(total as usize);
        let calls = Arc::new(AtomicUsize::new(0));
        let (fetchers, _) = fetchers(2, chunk, calls.clone());
        let reporter = Arc::new(Recorder::default());

        download_ranged(fetchers, sink, total, reporter.clone(), fast_config(chunk))
            .await
            .unwrap();

        let reports = reporter.reports.lock().await.clone();
        assert!(!reports.is_empty());
        assert!(
            reports.windows(2).all(|w| w[0] <= w[1]),
            "progress went backwards: {reports:?}"
        );
        assert_eq!(reports.last().copied(), Some(total));
    }

    #[tokio::test]
    async fn empty_file_succeeds_without_fetchers() {
        let sink = MemorySink::new(0);
        download_ranged(
            vec![],
            sink.clone(),
            0,
            Arc::new(Recorder::default()),
            fast_config(8),
        )
        .await
        .unwrap();
        assert!(sink.contents().await.is_empty());
    }

    #[tokio::test]
    async fn missing_fetchers_for_non_empty_file_is_an_error() {
        let sink = MemorySink::new(8);
        let err = download_ranged(
            vec![],
            sink,
            8,
            Arc::new(Recorder::default()),
            fast_config(8),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, RangedError::NoFetchers), "got {err}");
    }

    #[tokio::test]
    async fn file_sink_writes_offsets_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.bin");
        let sink = FileSink::create(&path, 10).await.unwrap();

        sink.write_at(4, &[9, 9, 9]).await.unwrap();
        sink.write_at(0, &[1, 2, 3, 4]).await.unwrap();
        sink.finish().await.unwrap();
        drop(sink);

        let bytes = tokio::fs::read(&path).await.unwrap();
        assert_eq!(bytes, vec![1, 2, 3, 4, 9, 9, 9, 0, 0, 0]);
    }
}
