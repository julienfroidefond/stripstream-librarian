//! The transfer-progress contract shared by every download flow.
//!
//! Four flows report progress — qBittorrent, AllDebrid's debrid-side magnet
//! status, AllDebrid's local magnet fetch, DDL providers and Telegram media — so
//! the meaning of the columns they write lives here, once:
//!
//! | value               | meaning                                              |
//! |---------------------|------------------------------------------------------|
//! | `bytes`             | bytes transferred so far                             |
//! | `progress`          | fraction of the transfer in `0..=1`, `0` when unknown |
//! | `download_speed`    | bytes per second                                     |
//! | `eta`               | seconds remaining                                    |
//! | the size column     | total size in bytes, and it is **never erased**      |
//!
//! The size rule is the one that used to differ per flow: `None` keeps the stored
//! value, `Some(0)` means "the backend does not know" and also keeps it, and only
//! a positive value replaces it. A backend that starts reporting a zero total
//! must not wipe a size an earlier poll already knew.
//!
//! A flow composes three pieces:
//!
//! * [`TransferRate`] smooths the raw byte samples into a speed;
//! * [`TransferProgress`] turns them into one snapshot with the single ETA
//!   formula (pick [`TransferProgress::from_bytes`] for a local transfer,
//!   [`TransferProgress::from_reported`] for values a remote backend reported);
//! * a [`ProgressSink`] adapter persists that snapshot in its own table, and
//!   [`Reporter`] plugs the whole thing into the ranged engine's
//!   [`ProgressReporter`] interface, throttling the writes to one per
//!   [`PROGRESS_UPDATE_BYTES`].

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::downloads::ranged::ProgressReporter;
use crate::downloads::torrent_ledger::{self, ProgressUpdate};

/// Progress is written to the DB at most every megabyte.
pub const PROGRESS_UPDATE_BYTES: i64 = 1_048_576;

/// Weight of the newest sample in the moving average.
const SMOOTHING: f64 = 0.4;

/// Byte counter that reports only every [`PROGRESS_UPDATE_BYTES`].
#[derive(Debug, Default)]
pub struct DownloadProgress {
    written: i64,
    reported: i64,
}

impl DownloadProgress {
    pub const fn new() -> Self {
        Self {
            written: 0,
            reported: 0,
        }
    }

    pub const fn written(&self) -> i64 {
        self.written
    }

    /// Adds `len` bytes and returns the new total when a DB update is due.
    pub fn advance(&mut self, len: i64) -> Option<i64> {
        self.written += len;
        if self.written - self.reported >= PROGRESS_UPDATE_BYTES {
            self.reported = self.written;
            return Some(self.written);
        }
        None
    }
}

/// Turns successive byte counts into a smoothed transfer rate and an ETA.
///
/// Raw deltas are noisy — parallel blocks finish in bursts, a `FLOOD_WAIT`
/// pauses everything — so callers get an exponential moving average instead of a
/// value that jumps on every report. Units are the same as `torrent_downloads`:
/// bytes per second for the rate, seconds for the ETA.
#[derive(Debug, Clone)]
pub struct TransferRate {
    last_bytes: i64,
    last_sample: Instant,
    speed: f64,
}

impl Default for TransferRate {
    fn default() -> Self {
        Self::new()
    }
}

impl TransferRate {
    pub fn new() -> Self {
        Self {
            last_bytes: 0,
            last_sample: Instant::now(),
            speed: 0.0,
        }
    }

    /// Records `bytes` written so far and returns the smoothed rate (bytes/s).
    pub fn sample(&mut self, bytes: i64) -> i64 {
        let elapsed = self.last_sample.elapsed();
        self.sample_after(bytes, elapsed)
    }

    /// Testable core of [`Self::sample`]: `elapsed` is the time since the
    /// previous sample.
    pub fn sample_after(&mut self, bytes: i64, elapsed: Duration) -> i64 {
        let delta = bytes - self.last_bytes;

        if delta < 0 {
            // The counter was reset (new attempt): no meaningful rate yet.
            self.last_bytes = bytes;
            self.last_sample = Instant::now();
            self.speed = 0.0;
            return 0;
        }

        let secs = elapsed.as_secs_f64();
        if secs <= 0.0 {
            // No measurable interval yet: keep the baseline so the bytes are
            // counted once a usable interval exists.
            return self.speed();
        }

        self.last_bytes = bytes;
        self.last_sample = Instant::now();

        let instant = delta as f64 / secs;
        self.speed = if self.speed <= 0.0 {
            instant
        } else {
            SMOOTHING * instant + (1.0 - SMOOTHING) * self.speed
        };
        self.speed()
    }

    /// Remaining seconds for `total` bytes when `bytes` have been transferred at
    /// the measured rate, or `0` when it cannot be estimated (unknown size,
    /// already complete, or no rate yet).
    pub fn eta(&self, bytes: i64, total: Option<i64>) -> i64 {
        remaining_eta(bytes, total, self.speed())
    }

    /// Current smoothed rate in bytes/s.
    pub fn speed(&self) -> i64 {
        self.speed.round() as i64
    }
}

/// The one ETA formula: `ceil(remaining / speed)` when the size and the rate are
/// both known, `0` otherwise.
fn remaining_eta(bytes: i64, total: Option<i64>, speed: i64) -> i64 {
    let Some(total) = known_total(total) else {
        return 0;
    };
    let remaining = total - bytes;
    if remaining <= 0 || speed <= 0 {
        return 0;
    }
    (remaining as f64 / speed as f64).ceil() as i64
}

/// A total size is only meaningful when positive; `0` and negatives mean the
/// backend does not know it.
fn known_total(total: Option<i64>) -> Option<i64> {
    total.filter(|total| *total > 0)
}

fn fraction(bytes: i64, total: Option<i64>) -> f32 {
    match known_total(total) {
        Some(total) => (bytes as f64 / total as f64).clamp(0.0, 1.0) as f32,
        None => 0.0,
    }
}

/// One progress snapshot, in the units of the columns described in the module
/// documentation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferProgress {
    pub bytes: i64,
    /// Total size in bytes; `None` when the transfer does not know it (which is
    /// never a reason to erase the stored one).
    pub total: Option<i64>,
    pub progress: f32,
    pub speed: i64,
    pub eta: i64,
}

impl TransferProgress {
    /// Snapshot of a local transfer: the fraction and the ETA are derived from
    /// `bytes`, `total` and the smoothed rate.
    ///
    /// The rate must have been sampled at `bytes` (as [`Reporter`] does) for the
    /// ETA to describe the current instant.
    pub fn from_bytes(bytes: i64, total: Option<i64>, rate: &TransferRate) -> Self {
        let total = known_total(total);
        Self {
            bytes,
            total,
            progress: fraction(bytes, total),
            speed: rate.speed(),
            eta: rate.eta(bytes, total),
        }
    }

    /// Snapshot of a remote backend's report.
    ///
    /// The backend's fraction and ETA are kept when it reports them; a missing
    /// fraction is derived from `bytes`/`total`, and a missing ETA from the
    /// remaining bytes and the reported speed, so every flow ends up on the same
    /// formula instead of its own.
    pub fn from_reported(
        bytes: i64,
        total: Option<i64>,
        progress: f32,
        speed: i64,
        eta: i64,
    ) -> Self {
        let total = known_total(total);
        let speed = speed.max(0);
        let reported = if progress > 0.0 {
            progress.clamp(0.0, 1.0)
        } else {
            fraction(bytes, total)
        };
        Self {
            bytes,
            total,
            // An unknown size means an unknown fraction, whatever was reported.
            progress: if total.is_some() { reported } else { 0.0 },
            speed,
            eta: if eta > 0 {
                eta
            } else {
                remaining_eta(bytes, total, speed)
            },
        }
    }
}

/// Where a snapshot is persisted.
///
/// A failure is returned, never panicked on: the caller treats progress as
/// best-effort and logs it, because a stale progress column must not fail a
/// download.
#[async_trait]
pub trait ProgressSink: Send + Sync {
    async fn report(&self, progress: &TransferProgress) -> sqlx::Result<()>;
}

/// `torrent_downloads`, through the ledger (qBittorrent, AllDebrid).
#[derive(Debug, Clone)]
pub struct TorrentSink {
    pool: PgPool,
    id: Uuid,
    /// Debrid peers seeding the torrent; `None` keeps the stored value.
    seeders: Option<i32>,
    /// Torrent name reported by the backend; `None` keeps the stored value.
    name: Option<String>,
}

impl TorrentSink {
    pub fn new(pool: PgPool, id: Uuid) -> Self {
        Self {
            pool,
            id,
            seeders: None,
            name: None,
        }
    }

    #[must_use]
    pub fn with_seeders(mut self, seeders: Option<i32>) -> Self {
        self.seeders = seeders;
        self
    }

    #[must_use]
    pub fn with_name(mut self, name: Option<&str>) -> Self {
        self.name = name.map(str::to_string);
        self
    }

    /// The ledger write for this snapshot.
    ///
    /// Public within the downloads module because arming the stall marker
    /// ([`torrent_ledger::arm_stall`]) writes the same columns without being a
    /// progress sink.
    pub(super) fn update<'a>(&'a self, progress: &TransferProgress) -> ProgressUpdate<'a> {
        ProgressUpdate {
            progress: progress.progress,
            speed: progress.speed,
            eta: progress.eta,
            file_size: progress.total,
            seeders: self.seeders,
            name: self.name.as_deref(),
            downloaded: progress.bytes,
        }
    }
}

#[async_trait]
impl ProgressSink for TorrentSink {
    async fn report(&self, progress: &TransferProgress) -> sqlx::Result<()> {
        torrent_ledger::record_progress(&self.pool, self.id, &self.update(progress)).await
    }
}

/// `ddl_provider_downloads`.
#[derive(Debug, Clone)]
pub struct DdlSink {
    pool: PgPool,
    id: Uuid,
}

impl DdlSink {
    pub fn new(pool: PgPool, id: Uuid) -> Self {
        Self { pool, id }
    }
}

#[async_trait]
impl ProgressSink for DdlSink {
    async fn report(&self, progress: &TransferProgress) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE ddl_provider_downloads \
             SET bytes_downloaded = $2, download_speed = $3, eta = $4, \
                 file_size = CASE WHEN $5::bigint IS NULL OR $5 = 0 THEN file_size ELSE $5 END, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(self.id)
        .bind(progress.bytes)
        .bind(progress.speed)
        .bind(progress.eta)
        .bind(progress.total)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// `telegram_book_links`.
#[derive(Debug, Clone)]
pub struct TelegramSink {
    pool: PgPool,
    id: Uuid,
}

impl TelegramSink {
    pub fn new(pool: PgPool, id: Uuid) -> Self {
        Self { pool, id }
    }
}

#[async_trait]
impl ProgressSink for TelegramSink {
    async fn report(&self, progress: &TransferProgress) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE telegram_book_links \
             SET bytes_downloaded = $2, download_speed = $3, eta = $4, \
                 file_size = CASE WHEN $5::bigint IS NULL OR $5 = 0 THEN file_size ELSE $5 END, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(self.id)
        .bind(progress.bytes)
        .bind(progress.speed)
        .bind(progress.eta)
        .bind(progress.total)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// The ranged engine's [`ProgressReporter`], composed over a sink.
///
/// The engine reports on every tick (or every block for the streaming
/// fallback); this throttles the database writes to one per
/// [`PROGRESS_UPDATE_BYTES`], samples the rate at the same instant so the speed
/// and the ETA describe the same window, and hands the snapshot to the sink.
pub struct Reporter {
    total: Option<i64>,
    state: Mutex<ReporterState>,
    sink: Box<dyn ProgressSink>,
}

struct ReporterState {
    bytes: DownloadProgress,
    rate: TransferRate,
}

impl Reporter {
    /// `total` is the known size, `None` for a transfer that does not know it
    /// (only the rate is then meaningful).
    pub fn new(total: Option<i64>, sink: Box<dyn ProgressSink>) -> Self {
        Self {
            total: known_total(total),
            state: Mutex::new(ReporterState {
                bytes: DownloadProgress::new(),
                rate: TransferRate::new(),
            }),
            sink,
        }
    }
}

#[async_trait]
impl ProgressReporter for Reporter {
    async fn report(&self, bytes_written: i64) {
        // Scope the guard so it is released before the `await`: a `MutexGuard`
        // held across an await point would make this future non-`Send` and break
        // the engine's `JoinSet`.
        let snapshot = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let delta = bytes_written - state.bytes.written();
            let Some(bytes) = state.bytes.advance(delta) else {
                return;
            };
            state.rate.sample(bytes);
            TransferProgress::from_bytes(bytes, self.total, &state.rate)
        };

        if let Err(err) = self.sink.report(&snapshot).await {
            tracing::debug!("download progress update failed: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: i64 = 1024 * 1024;

    #[test]
    fn first_sample_reports_the_instantaneous_rate() {
        let mut rate = TransferRate::new();
        assert_eq!(rate.sample_after(MB, Duration::from_secs(1)), MB);
    }

    #[test]
    fn successive_samples_are_smoothed() {
        let mut rate = TransferRate::new();
        assert_eq!(rate.sample_after(MB, Duration::from_secs(1)), MB);

        // A sudden burst to 3 MB/s only moves the average partway.
        let smoothed = rate.sample_after(4 * MB, Duration::from_secs(1));
        let expected = (SMOOTHING * 3.0 + (1.0 - SMOOTHING) * 1.0) * MB as f64;
        assert_eq!(smoothed, expected.round() as i64);
        assert!((1_700_000..1_900_000).contains(&smoothed));
    }

    #[test]
    fn eta_uses_the_smoothed_rate() {
        let mut rate = TransferRate::new();
        rate.sample_after(4 * MB, Duration::from_secs(2));
        assert_eq!(rate.speed(), 2 * MB);
        assert_eq!(rate.eta(4 * MB, Some(10 * MB)), 3);
    }

    #[test]
    fn eta_is_unknown_without_a_size_or_a_rate() {
        let mut rate = TransferRate::new();
        assert_eq!(rate.eta(0, None), 0);
        rate.sample_after(MB, Duration::from_secs(1));
        assert_eq!(rate.eta(MB, Some(0)), 0);
        assert_eq!(rate.eta(MB, Some(MB)), 0, "a complete transfer has no ETA");
    }

    #[test]
    fn a_reset_counter_clears_the_rate() {
        let mut rate = TransferRate::new();
        rate.sample_after(4 * MB, Duration::from_secs(2));
        assert!(rate.speed() > 0);

        assert_eq!(rate.sample_after(0, Duration::from_secs(1)), 0);
        assert_eq!(rate.speed(), 0);
        assert_eq!(rate.eta(0, Some(10 * MB)), 0);
    }

    #[test]
    fn a_zero_duration_never_divides_by_zero() {
        let mut rate = TransferRate::new();
        assert_eq!(rate.sample_after(MB, Duration::ZERO), 0);
        assert_eq!(
            rate.sample_after(2 * MB, Duration::from_secs(1)),
            2 * MB,
            "the first usable interval establishes the rate"
        );
    }

    #[test]
    fn progress_reports_at_most_every_megabyte() {
        let mut progress = DownloadProgress::new();
        assert_eq!(progress.advance(PROGRESS_UPDATE_BYTES - 1), None);
        assert_eq!(progress.advance(1), Some(PROGRESS_UPDATE_BYTES));
        assert_eq!(progress.advance(10), None);
        assert_eq!(progress.written(), PROGRESS_UPDATE_BYTES + 10);
        assert_eq!(
            progress.advance(PROGRESS_UPDATE_BYTES),
            Some(2 * PROGRESS_UPDATE_BYTES + 10)
        );
    }

    #[test]
    fn from_bytes_derives_the_fraction_speed_and_eta() {
        let mut rate = TransferRate::new();
        rate.sample_after(MB, Duration::from_secs(1));

        let snapshot = TransferProgress::from_bytes(MB, Some(4 * MB), &rate);
        assert_eq!(
            snapshot,
            TransferProgress {
                bytes: MB,
                total: Some(4 * MB),
                progress: 0.25,
                speed: MB,
                eta: 3,
            }
        );
    }

    #[test]
    fn from_bytes_without_a_size_keeps_the_rate_but_no_fraction_nor_eta() {
        let mut rate = TransferRate::new();
        rate.sample_after(MB, Duration::from_secs(1));

        let snapshot = TransferProgress::from_bytes(MB, None, &rate);
        assert_eq!(snapshot.total, None);
        assert_eq!(snapshot.progress, 0.0);
        assert_eq!(snapshot.speed, MB, "the rate is still meaningful");
        assert_eq!(snapshot.eta, 0);

        // A backend that claims a zero size is treated as not knowing it.
        assert_eq!(TransferProgress::from_bytes(MB, Some(0), &rate).total, None);
    }

    #[test]
    fn from_bytes_handles_a_complete_transfer_and_a_zero_rate() {
        let rate = TransferRate::new();
        let complete = TransferProgress::from_bytes(4 * MB, Some(4 * MB), &rate);
        assert_eq!(complete.progress, 1.0);
        assert_eq!(complete.eta, 0, "a complete transfer has no ETA");
        assert_eq!(complete.speed, 0);

        // Bytes beyond the reported total are clamped, never negative.
        assert_eq!(
            TransferProgress::from_bytes(5 * MB, Some(4 * MB), &rate).progress,
            1.0
        );
    }

    #[test]
    fn from_reported_keeps_the_backend_eta() {
        let snapshot = TransferProgress::from_reported(MB, Some(10 * MB), 0.1, 1000, 42);
        assert_eq!(snapshot.eta, 42, "a reported ETA wins over the local one");
        assert_eq!(snapshot.progress, 0.1);
        assert_eq!(snapshot.speed, 1000);
        assert_eq!(snapshot.total, Some(10 * MB));
    }

    #[test]
    fn from_reported_computes_a_missing_eta() {
        let snapshot = TransferProgress::from_reported(2 * MB, Some(10 * MB), 0.2, MB, 0);
        assert_eq!(snapshot.eta, 8);

        // A backend that reports neither an ETA nor a speed leaves it unknown.
        assert_eq!(
            TransferProgress::from_reported(2 * MB, Some(10 * MB), 0.2, 0, 0).eta,
            0
        );
    }

    #[test]
    fn from_reported_normalizes_what_the_backend_sent() {
        // An unknown size means an unknown fraction, whatever was reported.
        let snapshot = TransferProgress::from_reported(MB, None, 0.4, 100, 0);
        assert_eq!(snapshot.total, None);
        assert_eq!(snapshot.progress, 0.0);
        assert_eq!(snapshot.eta, 0);

        // A zero fraction with known bytes/total is derived instead.
        assert_eq!(
            TransferProgress::from_reported(MB, Some(4 * MB), 0.0, 0, 0).progress,
            0.25
        );

        // Out-of-range values are clamped or floored.
        let clamped = TransferProgress::from_reported(MB, Some(4 * MB), 3.0, -5, -2);
        assert_eq!(clamped.progress, 1.0);
        assert_eq!(clamped.speed, 0);
        assert_eq!(clamped.eta, 0);
    }

    // -----------------------------------------------------------------------
    // Sinks
    // -----------------------------------------------------------------------

    async fn seed_torrent_row(pool: &PgPool, status: &str, file_size: Option<i64>) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind(format!("lib_{library_id}"))
            .bind(format!("/libraries/{library_id}"))
            .execute(pool)
            .await
            .unwrap();

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO torrent_downloads (id, library_id, series_name, status, file_size) \
             VALUES ($1, $2, 'Aposimz', $3, $4)",
        )
        .bind(id)
        .bind(library_id)
        .bind(status)
        .bind(file_size)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn seed_ddl_row(pool: &PgPool, file_size: Option<i64>) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ddl_provider_downloads \
             (id, provider, external_id, filename, file_size, status) \
             VALUES ($1, 'fourtoutici', $2, 'Aposimz T01.cbz', $3, 'downloading')",
        )
        .bind(id)
        .bind(id.to_string())
        .bind(file_size)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn seed_telegram_link(pool: &PgPool, file_size: Option<i64>) -> Uuid {
        let source_id = Uuid::new_v4();
        sqlx::query("INSERT INTO telegram_sources (id, channel_username) VALUES ($1, $2)")
            .bind(source_id)
            .bind(format!("chan-{source_id}"))
            .execute(pool)
            .await
            .unwrap();

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO telegram_book_links \
             (id, source_id, message_id, filename, file_size, status) \
             VALUES ($1, $2, 1, 'Aposimz T01.cbz', $3, 'downloading')",
        )
        .bind(id)
        .bind(source_id)
        .bind(file_size)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    #[derive(sqlx::FromRow)]
    struct TorrentProgressRow {
        progress: f32,
        download_speed: i64,
        eta: i64,
        file_size: Option<i64>,
        seeders: Option<i32>,
        name: Option<String>,
        stall_started_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn torrent_sink_writes_the_progress_columns(pool: PgPool) {
        let id = seed_torrent_row(&pool, "downloading", Some(1000)).await;
        sqlx::query("UPDATE torrent_downloads SET stall_started_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        let snapshot = TransferProgress::from_bytes(500, Some(1000), &TransferRate::new());
        TorrentSink::new(pool.clone(), id)
            .with_seeders(Some(3))
            .with_name(Some("Aposimz T01"))
            .report(&snapshot)
            .await
            .unwrap();

        let row: TorrentProgressRow = sqlx::query_as(
            "SELECT progress, download_speed, eta, file_size, seeders, name, stall_started_at \
             FROM torrent_downloads WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.progress, 0.5);
        assert_eq!(row.download_speed, 0);
        assert_eq!(row.eta, 0);
        assert_eq!(row.file_size, Some(1000));
        assert_eq!(row.seeders, Some(3));
        assert_eq!(row.name.as_deref(), Some("Aposimz T01"));
        assert!(
            row.stall_started_at.is_none(),
            "a snapshot with transferred bytes clears the stall marker"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn torrent_sink_noops_once_the_row_left_downloading(pool: PgPool) {
        let id = seed_torrent_row(&pool, "completed", Some(1000)).await;

        let snapshot = TransferProgress::from_bytes(1000, Some(1000), &TransferRate::new());
        TorrentSink::new(pool.clone(), id)
            .report(&snapshot)
            .await
            .unwrap();

        let progress: f32 =
            sqlx::query_scalar("SELECT progress FROM torrent_downloads WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(progress, 0.0, "a finished row must not receive progress");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn ddl_sink_writes_the_progress_columns(pool: PgPool) {
        let id = seed_ddl_row(&pool, Some(1000)).await;

        let mut rate = TransferRate::new();
        rate.sample_after(500, Duration::from_secs(1));
        let snapshot = TransferProgress::from_bytes(500, Some(1000), &rate);
        DdlSink::new(pool.clone(), id)
            .report(&snapshot)
            .await
            .unwrap();

        let (bytes, speed, eta, file_size): (i64, i64, i64, Option<i64>) = sqlx::query_as(
            "SELECT bytes_downloaded, download_speed, eta, file_size \
             FROM ddl_provider_downloads WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!((bytes, speed, eta, file_size), (500, 500, 1, Some(1000)));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn telegram_sink_writes_the_progress_columns(pool: PgPool) {
        let id = seed_telegram_link(&pool, Some(1000)).await;

        let mut rate = TransferRate::new();
        rate.sample_after(250, Duration::from_secs(1));
        let snapshot = TransferProgress::from_bytes(250, Some(1000), &rate);
        TelegramSink::new(pool.clone(), id)
            .report(&snapshot)
            .await
            .unwrap();

        let (bytes, speed, eta, file_size): (i64, i64, i64, Option<i64>) = sqlx::query_as(
            "SELECT bytes_downloaded, download_speed, eta, file_size \
             FROM telegram_book_links WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!((bytes, speed, eta, file_size), (250, 250, 3, Some(1000)));
    }

    /// The size column is never erased: an unknown size (`None` or `Some(0)`)
    /// keeps whatever an earlier poll already knew, and only a positive value
    /// replaces it.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn no_sink_ever_erases_the_stored_size(pool: PgPool) {
        let torrent = seed_torrent_row(&pool, "downloading", Some(1000)).await;
        let ddl = seed_ddl_row(&pool, Some(1000)).await;
        let telegram = seed_telegram_link(&pool, Some(1000)).await;

        for total in [None, Some(0), Some(1000)] {
            TorrentSink::new(pool.clone(), torrent)
                .report(&TransferProgress::from_bytes(
                    0,
                    total,
                    &TransferRate::new(),
                ))
                .await
                .unwrap();
            DdlSink::new(pool.clone(), ddl)
                .report(&TransferProgress::from_bytes(
                    0,
                    total,
                    &TransferRate::new(),
                ))
                .await
                .unwrap();
            TelegramSink::new(pool.clone(), telegram)
                .report(&TransferProgress::from_bytes(
                    0,
                    total,
                    &TransferRate::new(),
                ))
                .await
                .unwrap();

            let torrent_size: Option<i64> =
                sqlx::query_scalar("SELECT file_size FROM torrent_downloads WHERE id = $1")
                    .bind(torrent)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            let ddl_size: Option<i64> =
                sqlx::query_scalar("SELECT file_size FROM ddl_provider_downloads WHERE id = $1")
                    .bind(ddl)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            let telegram_size: Option<i64> =
                sqlx::query_scalar("SELECT file_size FROM telegram_book_links WHERE id = $1")
                    .bind(telegram)
                    .fetch_one(&pool)
                    .await
                    .unwrap();

            assert_eq!(torrent_size, Some(1000), "torrent size for {total:?}");
            assert_eq!(ddl_size, Some(1000), "ddl size for {total:?}");
            assert_eq!(telegram_size, Some(1000), "telegram size for {total:?}");
        }

        // A positive size still replaces the stored one.
        let replaced = TransferProgress::from_bytes(0, Some(2_000), &TransferRate::new());
        TorrentSink::new(pool.clone(), torrent)
            .report(&replaced)
            .await
            .unwrap();
        DdlSink::new(pool.clone(), ddl)
            .report(&replaced)
            .await
            .unwrap();
        TelegramSink::new(pool.clone(), telegram)
            .report(&replaced)
            .await
            .unwrap();

        let sizes: Vec<Option<i64>> = sqlx::query_scalar(
            "SELECT file_size FROM torrent_downloads WHERE id = $1 \
             UNION ALL SELECT file_size FROM ddl_provider_downloads WHERE id = $2 \
             UNION ALL SELECT file_size FROM telegram_book_links WHERE id = $3",
        )
        .bind(torrent)
        .bind(ddl)
        .bind(telegram)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(sizes, vec![Some(2_000); 3]);
    }
}
