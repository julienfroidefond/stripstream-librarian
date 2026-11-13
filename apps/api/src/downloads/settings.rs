//! Download tuning, derived from the typed `limits` setting.
//!
//! Defaults and accepted ranges live in [`crate::settings::limits`]; this
//! module only turns the decoded [`Limits`] into the units the transfer paths
//! use (`usize` counts, `Duration`s).

use std::time::Duration;

use sqlx::PgPool;

use super::ranged::RangedConfig;
use super::telegram_monitor::MAX_CONCURRENT_CONNECTIONS;
use crate::settings::limits::Limits;

/// Block size for a direct (HTTP) ranged download.
///
/// See [`DownloadSettings::http_ranged_config`]: HTTP pays one request per block,
/// and too many requests on the same URL get the client rate-limited (fourtoutici
/// answers `403` to everything once that happens). 4 MiB keeps a 150 MB file at
/// ~40 requests while remaining a size ordinary download scripts stream in one go.
pub const HTTP_DDL_CHUNK_SIZE: i64 = 4 * 1024 * 1024;

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
        Self::from(&Limits::default())
    }
}

impl From<&Limits> for DownloadSettings {
    fn from(limits: &Limits) -> Self {
        Self {
            telegram_concurrency: limits.concurrent_telegram_downloads.get(),
            ddl_concurrency: limits.concurrent_ddl_provider_downloads.get(),
            alldebrid_concurrency: limits.concurrent_alldebrid_downloads.get(),
            telegram_connections: limits.telegram_download_connections.get(),
            http_connections: limits.ddl_download_connections.get(),
            max_attempts: limits.download_max_attempts.get(),
            stall_timeout: Duration::from_secs(limits.download_stall_timeout_seconds.get()),
            transfer_timeout: Duration::from_secs(limits.download_timeout_minutes.get() * 60),
            magnet_stall_timeout: Duration::from_secs(
                limits.alldebrid_stall_timeout_minutes.get() * 60,
            ),
        }
    }
}

impl DownloadSettings {
    /// Load the current values from the database, falling back to the defaults
    /// when the setting is missing or unreadable.
    pub async fn load(pool: &PgPool) -> Self {
        Self::from(&Limits::load(pool).await)
    }

    /// Connections a single download should open.
    ///
    /// Sockets are a process-wide resource. Letting `telegram_concurrency`
    /// downloads open `telegram_connections` sockets each would multiply the
    /// account's connection count and trigger flood waits as soon as a third
    /// download starts. Each download therefore gets at most its share of the
    /// global budget (never less than one), and the per-download setting still
    /// caps that share.
    pub fn telegram_sockets_per_download(&self) -> usize {
        let share = MAX_CONCURRENT_CONNECTIONS / self.telegram_concurrency.max(1);
        self.telegram_connections.min(share.max(1))
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

    /// Ranged tuning for direct (HTTP) downloads.
    ///
    /// The engine defaults are tuned for Telegram, where a block is one MTProto
    /// request carrying its own 4 KiB alignment. A DDL provider instead pays one
    /// HTTP request per block, and some hosts start answering `403` once a single
    /// URL has been requested a few dozen times in a row — a 150 MB file needs
    /// ~300 blocks at the 512 KiB default. Large blocks cut the request count by
    /// 8×, and a single worker per connection keeps concurrency to the configured
    /// connection count instead of doubling it.
    pub fn http_ranged_config(&self) -> RangedConfig {
        RangedConfig {
            chunk_size: HTTP_DDL_CHUNK_SIZE,
            workers_per_fetcher: 1,
            ..self.ranged_config()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn settings_of(value: Value) -> DownloadSettings {
        DownloadSettings::from(&Limits::parse(&value).unwrap())
    }

    #[test]
    fn defaults_come_from_the_limits_schema() {
        let default = DownloadSettings::default();
        assert_eq!(default.telegram_concurrency, 2);
        assert_eq!(default.ddl_concurrency, 2);
        assert_eq!(default.alldebrid_concurrency, 4);
        assert_eq!(default.telegram_connections, 4);
        assert_eq!(default.http_connections, 4);
        assert_eq!(default.max_attempts, 5);
        assert_eq!(default.stall_timeout, Duration::from_secs(180));
        assert_eq!(default.transfer_timeout, Duration::from_secs(30 * 60));
        assert_eq!(default.magnet_stall_timeout, Duration::from_secs(30 * 60));
    }

    #[test]
    fn reads_every_documented_field() {
        let settings = settings_of(json!({
            "concurrent_telegram_downloads": 3,
            "concurrent_ddl_provider_downloads": 5,
            "concurrent_alldebrid_downloads": 6,
            "telegram_download_connections": 7,
            "ddl_download_connections": 9,
            "download_max_attempts": 11,
            "download_stall_timeout_seconds": 240,
            "download_timeout_minutes": 90,
            "alldebrid_stall_timeout_minutes": 45,
        }));

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
    fn telegram_downloads_split_the_process_connection_budget() {
        let settings = |concurrency: u64, connections: u64| {
            settings_of(json!({
                "concurrent_telegram_downloads": concurrency,
                "telegram_download_connections": connections,
            }))
        };

        // Two downloads x 4 sockets fill the 8-socket budget.
        assert_eq!(settings(2, 4).telegram_sockets_per_download(), 4);
        // A third download takes a smaller share instead of queuing for a socket.
        assert_eq!(settings(3, 4).telegram_sockets_per_download(), 2);
        assert_eq!(settings(4, 8).telegram_sockets_per_download(), 2);
        // A download always gets at least one socket.
        assert_eq!(settings(16, 4).telegram_sockets_per_download(), 1);
        // The per-download setting still caps the share.
        assert_eq!(settings(2, 1).telegram_sockets_per_download(), 1);
    }

    #[test]
    fn ranged_config_threads_the_shared_knobs() {
        let config = settings_of(json!({
            "download_max_attempts": 9,
            "download_stall_timeout_seconds": 30,
        }))
        .ranged_config();
        assert_eq!(config.max_attempts, 9);
        assert_eq!(config.stall_timeout, Duration::from_secs(30));
        // The remaining knobs stay at the engine defaults.
        assert_eq!(config.chunk_size, 512 * 1024);
        assert_eq!(config.workers_per_fetcher, 2);
    }

    #[test]
    fn http_downloads_use_larger_blocks_and_one_worker_per_connection() {
        let settings = settings_of(json!({
            "download_max_attempts": 9,
            "download_stall_timeout_seconds": 30,
        }));

        let http = settings.http_ranged_config();
        assert_eq!(http.chunk_size, HTTP_DDL_CHUNK_SIZE);
        assert_eq!(http.workers_per_fetcher, 1);
        // Shared knobs still apply to the HTTP path.
        assert_eq!(http.max_attempts, 9);
        assert_eq!(http.stall_timeout, Duration::from_secs(30));

        // Telegram keeps the small blocks its 4 KiB alignment requires.
        assert_eq!(
            settings.ranged_config().chunk_size,
            RangedConfig::default().chunk_size
        );
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
        assert_eq!(
            settings.telegram_connections,
            DownloadSettings::default().telegram_connections
        );
    }
}
