//! Shared metadata logic used by metadata.rs (manual search) and metadata_batch/ (batch processing).

use sqlx::{PgPool, Row};

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

    if let Ok(Some(row)) =
        sqlx::query("SELECT value FROM app_settings WHERE key = 'metadata_providers'")
            .fetch_optional(pool)
            .await
    {
        let value: serde_json::Value = row.get("value");
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
    if let Ok(Some(row)) =
        sqlx::query("SELECT value FROM app_settings WHERE key = 'metadata_providers'")
            .fetch_optional(pool)
            .await
    {
        let value: serde_json::Value = row.get("value");
        if let Some(default) = value.get("default_provider").and_then(|v| v.as_str()) {
            if !default.is_empty() {
                return default.to_string();
            }
        }
    }

    "google_books".to_string()
}

/// Boost candidate confidence when local book count matches total_volumes.
///
/// - Exact match: +0.30
/// - Close match (±2): +0.15
///
/// Re-sorts candidates by confidence after boosting.
pub(crate) fn boost_confidence_by_book_count(
    candidates: &mut [metadata_providers::SeriesCandidate],
    local_count: i64,
) {
    if local_count <= 0 {
        return;
    }
    for c in candidates.iter_mut() {
        if let Some(ext_total) = c.total_volumes {
            if ext_total > 0 {
                if local_count == ext_total as i64 {
                    c.confidence = (c.confidence + 0.3).min(1.0);
                } else if (local_count - ext_total as i64).abs() <= 2 {
                    c.confidence = (c.confidence + 0.15).min(1.0);
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
