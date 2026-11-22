//! The release shape every download source writes to `available_downloads`.
//!
//! Prowlarr detection, the Prowlarr RSS poll and the DDL catalogue all produce
//! the same JSON rows; keeping the struct out of `detection` makes that shared
//! contract explicit and lets `ingest` own the publication steps.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AvailableReleaseDto {
    pub title: String,
    pub size: i64,
    pub download_url: Option<String>,
    pub indexer: Option<String>,
    pub seeders: Option<i32>,
    pub matched_missing_volumes: Vec<i32>,
    #[serde(default)]
    pub all_volumes: Vec<i32>,
    /// True if a previous download of overlapping volumes failed for this series.
    #[serde(default)]
    pub has_failed: bool,
    /// When this release was first detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_at: Option<String>,
    /// `high` can be automated; `review` has weaker title or volume evidence.
    #[serde(default = "default_match_confidence")]
    pub match_confidence: String,
    /// Machine-readable evidence used by the shared title matcher.
    #[serde(default = "default_match_reasons")]
    pub match_reasons: Vec<String>,
    /// Set when the release comes from a DDL provider rather than Prowlarr.
    /// The value is the provider registry id (e.g. `fourtoutici`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// DDL-provider-specific file id, used to start a direct download.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

fn default_match_confidence() -> String {
    // Historical JSON releases have no qualification evidence. Keep them
    // visible, but require an explicit confirmation until a scan refreshes
    // them with the shared matcher.
    "review".to_string()
}

fn default_match_reasons() -> Vec<String> {
    vec!["legacy_unqualified".to_string()]
}
