//! DDL providers: external catalogues exposed over plain HTTP.
//!
//! A "DDL provider" is a remote catalogue — a native JSON API or a scraped
//! HTML page — that can be searched for a series/tome and whose files are then
//! streamed straight into a library. This differs from the two existing download
//! paths: torrents go through qBittorrent and Telegram media through MTProto.
//!
//! DDL providers are toggled individually in settings through the
//! [`SETTINGS_KEY`] `app_settings` entry, and every download is tracked in the
//! `ddl_provider_downloads` table.

mod fourtoutici;
mod handlers;
mod search;
mod service;
mod types;

pub use handlers::*;
pub use search::process_ddl_search;
pub use types::*;

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::ApiError;

/// Settings key holding the per-provider configuration.
pub const SETTINGS_KEY: &str = "ddl_providers";

/// User-agent used for provider calls. Some catalogue front-ends reject
/// non-browser agents, so we advertise a browser regardless of the site.
pub const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Timeout for provider search/test requests.
pub(crate) const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(20);

/// Timeout for a whole file download.
pub(crate) const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Static metadata describing a provider.
#[derive(Debug, Clone)]
pub struct DdlProviderMeta {
    pub id: &'static str,
    pub label: &'static str,
    /// `json` for a native API, `html` for a scraped page.
    pub kind: &'static str,
    pub description: &'static str,
    pub requires_auth: bool,
    pub default_base_url: &'static str,
}

/// One catalogue entry returned by a DDL provider search.
#[derive(Debug, Clone)]
pub struct DdlProviderFile {
    pub external_id: String,
    /// Original filename, shown as-is in the UI.
    pub filename: String,
    /// Name used for series/volume matching when it differs from `filename`
    /// (e.g. a provider whose naming scheme the shared parser cannot read).
    pub match_name: Option<String>,
    pub size: Option<i64>,
}

/// Contract implemented by every DDL provider.
#[async_trait::async_trait]
pub trait DdlProvider: Send + Sync {
    fn meta(&self) -> DdlProviderMeta;

    /// Search the provider catalogue.
    async fn search(
        &self,
        base_url: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<DdlProviderFile>>;

    /// Absolute URL to stream a file from.
    fn file_url(&self, base_url: &str, external_id: &str) -> String;
}

/// All known providers.
pub fn registry() -> Vec<Box<dyn DdlProvider>> {
    vec![Box::new(fourtoutici::FourtouticiProvider)]
}

/// Look up a provider by its registry id.
pub fn get_ddl_provider(id: &str) -> Option<Box<dyn DdlProvider>> {
    registry().into_iter().find(|p| p.meta().id == id)
}

/// Persisted configuration for a single provider.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DdlProviderSettings {
    #[serde(default)]
    pub enabled: bool,
    /// Optional base-URL override (mirror / self-hosted instance).
    #[serde(default)]
    pub base_url: Option<String>,
}

/// The whole `ddl_providers` settings payload.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DdlProvidersConfig {
    #[serde(default)]
    pub providers: HashMap<String, DdlProviderSettings>,
    /// Cadence of the background search across enabled providers, in minutes.
    /// `0` (the default) disables it.
    #[serde(default)]
    pub search_interval_minutes: i32,
}

pub async fn load_config(pool: &sqlx::PgPool) -> Result<DdlProvidersConfig, ApiError> {
    stripstream_core::settings::load_setting(pool, SETTINGS_KEY)
        .await
        .map_err(|e| ApiError::internal(format!("invalid ddl_providers config: {e}")))
        .map(Option::unwrap_or_default)
}

pub async fn save_config(pool: &sqlx::PgPool, config: &DdlProvidersConfig) -> Result<(), ApiError> {
    let value = serde_json::to_value(config)
        .map_err(|e| ApiError::internal(format!("failed to encode config: {e}")))?;
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_at) VALUES ($1, $2, NOW()) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()",
    )
    .bind(SETTINGS_KEY)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Resolve the effective base URL for a provider (configured override or default).
pub fn resolve_base_url(config: &DdlProvidersConfig, meta: &DdlProviderMeta) -> String {
    config
        .providers
        .get(meta.id)
        .and_then(|s| s.base_url.clone())
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| meta.default_base_url.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Every enabled provider, paired with its resolved base URL.
pub fn enabled_providers(config: &DdlProvidersConfig) -> Vec<(Box<dyn DdlProvider>, String)> {
    registry()
        .into_iter()
        .filter_map(|provider| {
            let meta = provider.meta();
            config
                .providers
                .get(meta.id)
                .map(|s| s.enabled)
                .unwrap_or(false)
                .then(|| {
                    let base_url = resolve_base_url(config, &meta);
                    (provider, base_url)
                })
        })
        .collect()
}

/// Ensure the provider exists and is enabled, returning its handle and base URL.
pub(crate) fn require_enabled(
    config: &DdlProvidersConfig,
    id: &str,
) -> Result<(Box<dyn DdlProvider>, String), ApiError> {
    let provider = get_ddl_provider(id)
        .ok_or_else(|| ApiError::not_found(format!("unknown DDL provider: {id}")))?;
    let meta = provider.meta();
    let enabled = config
        .providers
        .get(meta.id)
        .map(|s| s.enabled)
        .unwrap_or(false);
    if !enabled {
        return Err(ApiError::bad_request(format!(
            "DDL provider '{id}' is disabled in settings"
        )));
    }
    let base_url = resolve_base_url(config, &meta);
    Ok((provider, base_url))
}
