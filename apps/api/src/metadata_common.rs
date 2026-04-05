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
/// - Exact match: +0.30
/// - Close match (±2): +0.15
/// Re-sorts candidates by confidence after boosting.
pub(crate) fn boost_confidence_by_book_count(
    candidates: &mut Vec<metadata_providers::SeriesCandidate>,
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
mod tests {
    use super::*;

    fn make_candidate(title: &str, total_volumes: Option<i32>, confidence: f32) -> metadata_providers::SeriesCandidate {
        metadata_providers::SeriesCandidate {
            external_id: format!("test:{title}"),
            title: title.to_string(),
            authors: vec![],
            description: None,
            publishers: vec![],
            start_year: None,
            total_volumes,
            cover_url: None,
            external_url: None,
            confidence,
            metadata_json: serde_json::json!({}),
        }
    }

    #[test]
    fn boost_exact_match_increases_confidence() {
        let mut candidates = vec![
            make_candidate("Naruto", Some(72), 0.7),
            make_candidate("Naruto (Édition Hokage)", Some(35), 0.5),
        ];
        boost_confidence_by_book_count(&mut candidates, 72);
        assert!((candidates[0].confidence - 1.0).abs() < f32::EPSILON);
        assert!((candidates[1].confidence - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn boost_close_match_moderate_increase() {
        let mut candidates = vec![make_candidate("Series", Some(10), 0.6)];
        boost_confidence_by_book_count(&mut candidates, 11);
        assert!((candidates[0].confidence - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn boost_no_match_unchanged() {
        let mut candidates = vec![make_candidate("Series", Some(10), 0.6)];
        boost_confidence_by_book_count(&mut candidates, 50);
        assert!((candidates[0].confidence - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn boost_none_total_volumes_unchanged() {
        let mut candidates = vec![make_candidate("Series", None, 0.8)];
        boost_confidence_by_book_count(&mut candidates, 10);
        assert!((candidates[0].confidence - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn boost_reorders_by_confidence() {
        let mut candidates = vec![
            make_candidate("Edition A", Some(35), 0.6),
            make_candidate("Edition B", Some(72), 0.5),
        ];
        boost_confidence_by_book_count(&mut candidates, 72);
        assert_eq!(candidates[0].title, "Edition B");
    }

    #[test]
    fn boost_capped_at_one() {
        let mut candidates = vec![make_candidate("Series", Some(10), 0.9)];
        boost_confidence_by_book_count(&mut candidates, 10);
        assert!((candidates[0].confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn boost_zero_local_count_unchanged() {
        let mut candidates = vec![make_candidate("Series", Some(10), 0.7)];
        boost_confidence_by_book_count(&mut candidates, 0);
        assert!((candidates[0].confidence - 0.7).abs() < f32::EPSILON);
    }
}
