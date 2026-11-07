//! Transfer-rate smoothing shared by the Telegram and DDL download workers.
//!
//! Both flows report the number of written bytes at a regular cadence (once a
//! second for the ranged engine, once per megabyte for the single-stream
//! fallback). Raw deltas are noisy — parallel blocks finish in bursts, a
//! `FLOOD_WAIT` pauses everything — so the UI receives an exponential moving
//! average instead of a value that jumps on every report.

use std::time::{Duration, Instant};

/// Weight of the newest sample in the moving average.
const SMOOTHING: f64 = 0.4;

/// Turns successive byte counts into a smoothed transfer rate and an ETA.
///
/// Units are the same as `torrent_downloads`: bytes per second for the rate,
/// seconds for the ETA.
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

    /// Remaining seconds for `total` bytes, or `0` when it cannot be estimated
    /// (unknown size, already complete, or no rate yet).
    pub fn eta(&self, total: Option<i64>) -> i64 {
        let Some(total) = total.filter(|total| *total > 0) else {
            return 0;
        };
        let remaining = total - self.last_bytes;
        if remaining <= 0 || self.speed <= 0.0 {
            return 0;
        }
        (remaining as f64 / self.speed).ceil() as i64
    }

    /// Current smoothed rate in bytes/s.
    pub fn speed(&self) -> i64 {
        self.speed.round() as i64
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
        assert_eq!(rate.eta(Some(10 * MB)), 3);
    }

    #[test]
    fn eta_is_unknown_without_a_size_or_a_rate() {
        let mut rate = TransferRate::new();
        assert_eq!(rate.eta(None), 0);
        rate.sample_after(MB, Duration::from_secs(1));
        assert_eq!(rate.eta(Some(0)), 0);
        assert_eq!(rate.eta(Some(MB)), 0, "a complete transfer has no ETA");
    }

    #[test]
    fn a_reset_counter_clears_the_rate() {
        let mut rate = TransferRate::new();
        rate.sample_after(4 * MB, Duration::from_secs(2));
        assert!(rate.speed() > 0);

        assert_eq!(rate.sample_after(0, Duration::from_secs(1)), 0);
        assert_eq!(rate.speed(), 0);
        assert_eq!(rate.eta(Some(10 * MB)), 0);
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
}
