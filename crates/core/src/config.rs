use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub listen_addr: String,
    pub database_url: String,
    pub meili_url: String,
    pub meili_master_key: String,
    pub api_bootstrap_token: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: std::env::var("API_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            meili_url: std::env::var("MEILI_URL").context("MEILI_URL is required")?,
            meili_master_key: std::env::var("MEILI_MASTER_KEY").context("MEILI_MASTER_KEY is required")?,
            api_bootstrap_token: std::env::var("API_BOOTSTRAP_TOKEN")
                .context("API_BOOTSTRAP_TOKEN is required")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub listen_addr: String,
    pub database_url: String,
    pub meili_url: String,
    pub meili_master_key: String,
    pub scan_interval_seconds: u64,
}

impl IndexerConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: std::env::var("INDEXER_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8081".to_string()),
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            meili_url: std::env::var("MEILI_URL").context("MEILI_URL is required")?,
            meili_master_key: std::env::var("MEILI_MASTER_KEY").context("MEILI_MASTER_KEY is required")?,
            scan_interval_seconds: std::env::var("INDEXER_SCAN_INTERVAL_SECONDS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5),
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
                .unwrap_or_else(|_| "0.0.0.0:8082".to_string()),
            api_base_url: std::env::var("API_BASE_URL").unwrap_or_else(|_| "http://api:8080".to_string()),
            api_token: std::env::var("API_BOOTSTRAP_TOKEN").context("API_BOOTSTRAP_TOKEN is required")?,
        })
    }
}
