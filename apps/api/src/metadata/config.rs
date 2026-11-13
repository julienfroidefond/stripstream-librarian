//! Shared metadata logic used by metadata.rs (manual search) and metadata_batch/ (batch processing).

use sqlx::PgPool;
use stripstream_core::settings::load_setting;

use crate::metadata_providers;

/// Load provider configuration from DB (API keys, language).
/// Unified version replacing duplicate implementations in metadata.rs and metadata_batch.rs.
pub(crate) async fn load_provider_config(
    pool: &PgPool,
    provider_name: &str,
) -> metadata_providers::ProviderConfig {
    let mut config = metadata_providers::ProviderConfig {
        language: "en".to_string(),
        ..Default::default()
    };

    if let Some(value) = load_setting::<serde_json::Value>(pool, "metadata_providers")
        .await
        .ok()
        .flatten()
    {
        if let Some(api_key) = value
            .get(provider_name)
            .and_then(|p| p.get("api_key"))
            .and_then(|k| k.as_str())
        {
            if !api_key.is_empty() {
                config.api_key = Some(api_key.to_string());
            }
        }
        if let Some(lang) = value.get("metadata_language").and_then(|l| l.as_str()) {
            if !lang.is_empty() {
                config.language = lang.to_string();
            }
        }
    }

    config
}

/// Resolve the provider name: library-level override → global setting → "google_books" default.
pub(crate) async fn resolve_provider_name(pool: &PgPool, lib_provider: Option<&str>) -> String {
    if let Some(p) = lib_provider {
        if !p.is_empty() {
            return p.to_string();
        }
    }

    // Check global setting
    if let Some(value) = load_setting::<serde_json::Value>(pool, "metadata_providers")
        .await
        .ok()
        .flatten()
    {
        if let Some(default) = value.get("default_provider").and_then(|v| v.as_str()) {
            if !default.is_empty() {
                return default.to_string();
            }
        }
    }

    "google_books".to_string()
}
