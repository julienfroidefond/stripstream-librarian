//! Download tuning, read from the `limits` application setting.
//!
//! Every knob has a default equal to the constant the code used before it became
//! configurable, so an installation that never opens the Backoffice behaves
//! exactly as before. Values are clamped on read: a stored `0` would otherwise
//! build a `Semaphore::new(0)` (every download blocked forever) or a zero-length
//! stall timeout (every transfer aborted at once).

use std::time::Duration;

use serde_json::{Map, Value};
use sqlx::PgPool;
use stripstream_core::settings::load_setting;

use super::ranged::RangedConfig;

/// Connections opened per Telegram download. One Telegram socket caps around
/// 2 MB/s no matter the local bandwidth, so spreading a download over a handful
/// of them is what makes large files finish in reasonable time.
pub const DEFAULT_TELEGRAM_CONNECTIONS: usize = 4;

/// Upper bound on the number of Telegram connections: past this Telegram
/// throttles and throughput collapses.
pub const MAX_TELEGRAM_CONNECTIONS: usize = 8;

/// Connections opened per direct (HTTP) download.
pub const DEFAULT_HTTP_CONNECTIONS: usize = 4;

/// Upper bound on direct-download connections.
pub const MAX_HTTP_CONNECTIONS: usize = 16;

/// Highest accepted value for any per-queue download concurrency.
pub const MAX_DOWNLOAD_CONCURRENCY: usize = 16;

/// Blocks are retried up to this many times before a transfer is abandoned.
pub const DEFAULT_MAX_ATTEMPTS: usize = 5;
pub const MAX_MAX_ATTEMPTS: usize = 20;

/// A single ranged request may stay silent this long before it is retried.
pub const DEFAULT_STALL_TIMEOUT_SECS: u64 = 180;
pub const MIN_STALL_TIMEOUT_SECS: u64 = 10;
pub const MAX_STALL_TIMEOUT_SECS: u64 = 3600;

/// Whole-transfer ceiling for a direct (HTTP) download.
pub const DEFAULT_TRANSFER_TIMEOUT_MINUTES: u64 = 30;
pub const MAX_TRANSFER_TIMEOUT_MINUTES: u64 = 24 * 60;

/// An in-flight AllDebrid magnet that stays at zero bytes this long is abandoned
/// as unreachable (dead torrent or blocked tracker).
pub const DEFAULT_MAGNET_STALL_TIMEOUT_MINUTES: u64 = 30;
pub const MAX_MAGNET_STALL_TIMEOUT_MINUTES: u64 = 24 * 60;

/// Everything the download paths read from `app_settings.limits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadSettings {
    /// Concurrent Telegram book downloads.
    pub telegram_concurrency: usize,
    /// Concurrent direct (DDL provider) downloads.
    pub ddl_concurrency: usize,
    /// Concurrent AllDebrid magnet transfers in the poller.
    pub alldebrid_concurrency: usize,
    /// Connections opened per Telegram download.
    pub telegram_connections: usize,
    /// Connections opened per direct (HTTP) download.
    pub http_connections: usize,
    /// Block retries per transfer.
    pub max_attempts: usize,
    /// Inactivity bound for a single ranged request.
    pub stall_timeout: Duration,
    /// Whole-transfer ceiling for a direct download.
    pub transfer_timeout: Duration,
    /// How long an AllDebrid magnet may stay at zero bytes.
    pub magnet_stall_timeout: Duration,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            telegram_concurrency: 2,
            ddl_concurrency: 2,
            alldebrid_concurrency: 4,
            telegram_connections: DEFAULT_TELEGRAM_CONNECTIONS,
            http_connections: DEFAULT_HTTP_CONNECTIONS,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            stall_timeout: Duration::from_secs(DEFAULT_STALL_TIMEOUT_SECS),
            transfer_timeout: Duration::from_secs(DEFAULT_TRANSFER_TIMEOUT_MINUTES * 60),
            magnet_stall_timeout: Duration::from_secs(DEFAULT_MAGNET_STALL_TIMEOUT_MINUTES * 60),
        }
    }
}

impl DownloadSettings {
    /// Build from a decoded `limits` value. Pure, so it can be unit-tested;
    /// unknown or malformed fields fall back to the default and every accepted
    /// value is clamped to its documented range.
    pub fn from_limits(value: Option<&Value>) -> Self {
        let default = Self::default();
        let Some(obj) = value.and_then(Value::as_object) else {
            return default;
        };

        let concurrency =
            |key, fallback| usize_field(obj, key, fallback, 1, MAX_DOWNLOAD_CONCURRENCY);

        Self {
            telegram_concurrency: concurrency(
                "concurrent_telegram_downloads",
                default.telegram_concurrency,
            ),
            ddl_concurrency: concurrency(
                "concurrent_ddl_provider_downloads",
                default.ddl_concurrency,
            ),
            alldebrid_concurrency: concurrency(
                "concurrent_alldebrid_downloads",
                default.alldebrid_concurrency,
            ),
            telegram_connections: usize_field(
                obj,
                "telegram_download_connections",
                default.telegram_connections,
                1,
                MAX_TELEGRAM_CONNECTIONS,
            ),
            http_connections: usize_field(
                obj,
                "ddl_download_connections",
                default.http_connections,
                1,
                MAX_HTTP_CONNECTIONS,
            ),
            max_attempts: usize_field(
                obj,
                "download_max_attempts",
                default.max_attempts,
                1,
                MAX_MAX_ATTEMPTS,
            ),
            stall_timeout: Duration::from_secs(u64_field(
                obj,
                "download_stall_timeout_seconds",
                default.stall_timeout.as_secs(),
                MIN_STALL_TIMEOUT_SECS,
                MAX_STALL_TIMEOUT_SECS,
            )),
            transfer_timeout: Duration::from_secs(
                u64_field(
                    obj,
                    "download_timeout_minutes",
                    default.transfer_timeout.as_secs() / 60,
                    1,
                    MAX_TRANSFER_TIMEOUT_MINUTES,
                ) * 60,
            ),
            magnet_stall_timeout: Duration::from_secs(
                u64_field(
                    obj,
                    "alldebrid_stall_timeout_minutes",
                    default.magnet_stall_timeout.as_secs() / 60,
                    1,
                    MAX_MAGNET_STALL_TIMEOUT_MINUTES,
                ) * 60,
            ),
        }
    }

    /// Load the current values from the database, falling back to the defaults
    /// when the setting is missing or unreadable.
    pub async fn load(pool: &PgPool) -> Self {
        let value = load_setting::<Value>(pool, "limits").await.ok().flatten();
        Self::from_limits(value.as_ref())
    }

    /// Ranged-engine tuning shared by the transfer paths: block retries and the
    /// per-request stall timeout. The remaining knobs keep their defaults.
    pub fn ranged_config(&self) -> RangedConfig {
        RangedConfig {
            max_attempts: self.max_attempts,
            stall_timeout: self.stall_timeout,
            ..RangedConfig::default()
        }
    }
}

fn usize_field(
    obj: &Map<String, Value>,
    key: &str,
    fallback: usize,
    min: usize,
    max: usize,
) -> usize {
    obj.get(key)
        .and_then(Value::as_u64)
        .map(|v| (v as usize).clamp(min, max))
        .unwrap_or(fallback)
}

fn u64_field(obj: &Map<String, Value>, key: &str, fallback: u64, min: u64, max: u64) -> u64 {
    obj.get(key)
        .and_then(Value::as_u64)
        .map(|v| v.clamp(min, max))
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_or_malformed_limits_fall_back_to_defaults() {
        let default = DownloadSettings::default();
        assert_eq!(DownloadSettings::from_limits(None), default);
        assert_eq!(
            DownloadSettings::from_limits(Some(&json!("not an object"))),
            default
        );
        // Wrong types are ignored field by field.
        assert_eq!(
            DownloadSettings::from_limits(Some(&json!({
                "concurrent_telegram_downloads": "lots",
                "ddl_download_connections": -1,
            }))),
            default
        );
    }

    #[test]
    fn reads_every_documented_field() {
        let settings = DownloadSettings::from_limits(Some(&json!({
            "concurrent_telegram_downloads": 3,
            "concurrent_ddl_provider_downloads": 5,
            "concurrent_alldebrid_downloads": 6,
            "telegram_download_connections": 7,
            "ddl_download_connections": 9,
            "download_max_attempts": 11,
            "download_stall_timeout_seconds": 240,
            "download_timeout_minutes": 90,
            "alldebrid_stall_timeout_minutes": 45,
        })));

        assert_eq!(settings.telegram_concurrency, 3);
        assert_eq!(settings.ddl_concurrency, 5);
        assert_eq!(settings.alldebrid_concurrency, 6);
        assert_eq!(settings.telegram_connections, 7);
        assert_eq!(settings.http_connections, 9);
        assert_eq!(settings.max_attempts, 11);
        assert_eq!(settings.stall_timeout, Duration::from_secs(240));
        assert_eq!(settings.transfer_timeout, Duration::from_secs(90 * 60));
        assert_eq!(settings.magnet_stall_timeout, Duration::from_secs(45 * 60));
    }

    #[test]
    fn clamps_every_field_into_its_range() {
        // A stored 0 must never produce `Semaphore::new(0)` / a zero timeout.
        let low = DownloadSettings::from_limits(Some(&json!({
            "concurrent_telegram_downloads": 0,
            "concurrent_ddl_provider_downloads": 0,
            "concurrent_alldebrid_downloads": 0,
            "telegram_download_connections": 0,
            "ddl_download_connections": 0,
            "download_max_attempts": 0,
            "download_stall_timeout_seconds": 0,
            "download_timeout_minutes": 0,
            "alldebrid_stall_timeout_minutes": 0,
        })));
        assert_eq!(low.telegram_concurrency, 1);
        assert_eq!(low.ddl_concurrency, 1);
        assert_eq!(low.alldebrid_concurrency, 1);
        assert_eq!(low.telegram_connections, 1);
        assert_eq!(low.http_connections, 1);
        assert_eq!(low.max_attempts, 1);
        assert_eq!(
            low.stall_timeout,
            Duration::from_secs(MIN_STALL_TIMEOUT_SECS)
        );
        assert_eq!(low.transfer_timeout, Duration::from_secs(60));

        let high = DownloadSettings::from_limits(Some(&json!({
            "concurrent_telegram_downloads": 9999,
            "concurrent_ddl_provider_downloads": 9999,
            "concurrent_alldebrid_downloads": 9999,
            "telegram_download_connections": 9999,
            "ddl_download_connections": 9999,
            "download_max_attempts": 9999,
            "download_stall_timeout_seconds": 999_999,
            "download_timeout_minutes": 999_999,
            "alldebrid_stall_timeout_minutes": 999_999,
        })));
        assert_eq!(high.telegram_concurrency, MAX_DOWNLOAD_CONCURRENCY);
        assert_eq!(high.ddl_concurrency, MAX_DOWNLOAD_CONCURRENCY);
        assert_eq!(high.alldebrid_concurrency, MAX_DOWNLOAD_CONCURRENCY);
        assert_eq!(high.telegram_connections, MAX_TELEGRAM_CONNECTIONS);
        assert_eq!(high.http_connections, MAX_HTTP_CONNECTIONS);
        assert_eq!(high.max_attempts, MAX_MAX_ATTEMPTS);
        assert_eq!(
            high.stall_timeout,
            Duration::from_secs(MAX_STALL_TIMEOUT_SECS)
        );
        assert_eq!(
            high.transfer_timeout,
            Duration::from_secs(MAX_TRANSFER_TIMEOUT_MINUTES * 60)
        );
        assert_eq!(
            high.magnet_stall_timeout,
            Duration::from_secs(MAX_MAGNET_STALL_TIMEOUT_MINUTES * 60)
        );
    }

    #[test]
    fn ranged_config_threads_the_shared_knobs() {
        let config = DownloadSettings::from_limits(Some(&json!({
            "download_max_attempts": 9,
            "download_stall_timeout_seconds": 30,
        })))
        .ranged_config();
        assert_eq!(config.max_attempts, 9);
        assert_eq!(config.stall_timeout, Duration::from_secs(30));
        // The remaining knobs stay at the engine defaults.
        assert_eq!(config.chunk_size, 512 * 1024);
        assert_eq!(config.workers_per_fetcher, 2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn load_reads_the_limits_setting(pool: PgPool) {
        assert_eq!(
            DownloadSettings::load(&pool).await,
            DownloadSettings::default()
        );

        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) \
             VALUES ('limits', $1, NOW()) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(json!({
            "concurrent_ddl_provider_downloads": 6,
            "ddl_download_connections": 3,
        }))
        .execute(&pool)
        .await
        .unwrap();

        let settings = DownloadSettings::load(&pool).await;
        assert_eq!(settings.ddl_concurrency, 6);
        assert_eq!(settings.http_connections, 3);
        // Untouched keys keep their defaults.
        assert_eq!(settings.telegram_connections, DEFAULT_TELEGRAM_CONNECTIONS);
    }
}
