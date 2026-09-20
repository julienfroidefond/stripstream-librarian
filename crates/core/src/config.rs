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
