use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub listen_addr: String,
    pub database_url: String,
    pub api_bootstrap_token: String,
    pub db_max_connections: u32,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: std::env::var("API_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:7080".to_string()),
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            api_bootstrap_token: std::env::var("API_BOOTSTRAP_TOKEN")
                .context("API_BOOTSTRAP_TOKEN is required")?,
            db_max_connections: env_or("API_DB_MAX_CONNECTIONS", 10),
        })
    }
}

#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub listen_addr: String,
    pub database_url: String,
    pub scan_interval_seconds: u64,
    pub thumbnail_config: ThumbnailConfig,
}

#[derive(Debug, Clone)]
pub struct ThumbnailConfig {
    pub enabled: bool,
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub format: String,
    pub directory: String,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 300,
            height: 400,
            quality: 80,
            format: "webp".to_string(),
            directory: "/data/thumbnails".to_string(),
        }
    }
}

/// Parse an environment variable with a fallback default value.
pub fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Parse an environment variable as a String with a fallback default.
pub fn env_string_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

impl IndexerConfig {
    pub fn from_env() -> Result<Self> {
        let mut thumbnail_config = ThumbnailConfig::default();
        thumbnail_config.enabled = env_or("THUMBNAIL_ENABLED", thumbnail_config.enabled);
        thumbnail_config.width = env_or("THUMBNAIL_WIDTH", thumbnail_config.width);
        thumbnail_config.height = env_or("THUMBNAIL_HEIGHT", thumbnail_config.height);
        thumbnail_config.quality = env_or("THUMBNAIL_QUALITY", thumbnail_config.quality);
        thumbnail_config.format = env_string_or("THUMBNAIL_FORMAT", &thumbnail_config.format);
        thumbnail_config.directory =
            env_string_or("THUMBNAIL_DIRECTORY", &thumbnail_config.directory);

        Ok(Self {
            listen_addr: env_string_or("INDEXER_LISTEN_ADDR", "0.0.0.0:7081"),
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            scan_interval_seconds: env_or("INDEXER_SCAN_INTERVAL_SECONDS", 5),
            thumbnail_config,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AdminUiConfig {
    pub listen_addr: String,
    pub api_base_url: String,
    pub api_token: String,
}

impl AdminUiConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: std::env::var("ADMIN_UI_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:7082".to_string()),
            api_base_url: std::env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://api:7080".to_string()),
            api_token: std::env::var("API_BOOTSTRAP_TOKEN")
                .context("API_BOOTSTRAP_TOKEN is required")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Environment variables are process-global; serialise every test that mutates them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear(keys: &[&str]) {
        for key in keys {
            std::env::remove_var(key);
        }
    }

    /// Run `f` with `vars` set, restoring the previous environment afterwards so
    /// process-global variables are not left clobbered for other tests.
    fn with_env<F: FnOnce()>(vars: &[(&str, &str)], f: F) {
        let previous: Vec<(&str, Option<std::ffi::OsString>)> = vars
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect();
        for (key, value) in vars {
            std::env::set_var(key, value);
        }

        f();

        for (key, old) in previous {
            match old {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    fn env_or_falls_back_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear(&["STL_TEST_ENV_OR_UNSET"]);

        assert_eq!(env_or::<u32>("STL_TEST_ENV_OR_UNSET", 42), 42);
    }

    #[test]
    fn env_or_parses_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("STL_TEST_ENV_OR_OVERRIDE", "7");

        let value = env_or::<u32>("STL_TEST_ENV_OR_OVERRIDE", 10);
        clear(&["STL_TEST_ENV_OR_OVERRIDE"]);

        assert_eq!(value, 7);
    }

    #[test]
    fn env_or_falls_back_on_unparsable_value() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("STL_TEST_ENV_OR_INVALID", "not-a-number");

        let value = env_or::<u32>("STL_TEST_ENV_OR_INVALID", 10);
        clear(&["STL_TEST_ENV_OR_INVALID"]);

        assert_eq!(value, 10);
    }

    #[test]
    fn env_string_or_returns_default_then_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear(&["STL_TEST_ENV_STRING_OR"]);

        let default_value = env_string_or("STL_TEST_ENV_STRING_OR", "fallback");
        std::env::set_var("STL_TEST_ENV_STRING_OR", "custom");
        let override_value = env_string_or("STL_TEST_ENV_STRING_OR", "fallback");
        clear(&["STL_TEST_ENV_STRING_OR"]);

        assert_eq!(default_value, "fallback");
        assert_eq!(override_value, "custom");
    }

    #[test]
    fn api_config_reads_pool_size_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();

        with_env(
            &[
                ("DATABASE_URL", "postgres://example/db"),
                ("API_BOOTSTRAP_TOKEN", "token"),
                ("API_DB_MAX_CONNECTIONS", "25"),
            ],
            || {
                let config = ApiConfig::from_env().unwrap();
                assert_eq!(config.db_max_connections, 25);
            },
        );
    }

    #[test]
    fn api_config_defaults_pool_size_to_ten() {
        let _guard = ENV_LOCK.lock().unwrap();

        with_env(
            &[
                ("DATABASE_URL", "postgres://example/db"),
                ("API_BOOTSTRAP_TOKEN", "token"),
            ],
            || {
                clear(&["API_DB_MAX_CONNECTIONS"]);
                let config = ApiConfig::from_env().unwrap();
                assert_eq!(config.db_max_connections, 10);
            },
        );
    }
}
