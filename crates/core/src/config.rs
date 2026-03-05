use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub listen_addr: String,
    pub database_url: String,
    pub api_bootstrap_token: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: std::env::var("API_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            api_bootstrap_token: std::env::var("API_BOOTSTRAP_TOKEN")
                .context("API_BOOTSTRAP_TOKEN is required")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub listen_addr: String,
}

impl IndexerConfig {
    pub fn from_env() -> Self {
        Self {
            listen_addr: std::env::var("INDEXER_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8081".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdminUiConfig {
    pub listen_addr: String,
}

impl AdminUiConfig {
    pub fn from_env() -> Self {
        Self {
            listen_addr: std::env::var("ADMIN_UI_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8082".to_string()),
        }
    }
}
