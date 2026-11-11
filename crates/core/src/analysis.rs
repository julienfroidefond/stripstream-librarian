//! Explicit lifecycle of a book's archive analysis.
//!
//! Discovery inserts books as [`AnalysisStatus::Discovered`]. The indexer claims
//! them as [`AnalysisStatus::Analyzing`], then moves them to `Complete` when the
//! page count and cover are both known, `CoverPending` when only the page count
//! could be recovered, or `Failed` when no page count could be read.
//!
//! `Analyzing` is a valid work input again: a crashed or cancelled job leaves
//! rows in that state and the next analysis pass retries them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    /// Indexed by discovery, archive never opened.
    Discovered,
    /// Claimed by an analysis job that has not recorded an outcome yet.
    Analyzing,
    /// Page count known, thumbnail still to generate.
    CoverPending,
    /// Page count and thumbnail both known.
    Complete,
    /// Analysis produced no page count; retryable.
    Failed,
}

impl AnalysisStatus {
    pub const ALL: [Self; 5] = [
        Self::Discovered,
        Self::Analyzing,
        Self::CoverPending,
        Self::Complete,
        Self::Failed,
    ];

    /// Stable database representation used by `books.analysis_status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Analyzing => "analyzing",
            Self::CoverPending => "cover_pending",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    /// Parse a value read from `books.analysis_status`.
    pub fn from_db(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
    }

    /// States that diagnostics report as "unanalysed".
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Discovered | Self::Analyzing)
    }

    /// States a full analysis job can pick up.
    pub const fn is_analysis_candidate(self) -> bool {
        matches!(self, Self::Discovered | Self::Analyzing | Self::Failed)
    }

    /// States a thumbnail-only job can pick up.
    pub const fn is_thumbnail_candidate(self) -> bool {
        matches!(self, Self::CoverPending)
    }

    /// State reached after an extraction attempt.
    ///
    /// `has_thumbnail` describes the book at extraction time: when the archive
    /// already had a thumbnail, recovering the page count is enough to finish.
    pub const fn after_extraction(page_count: Option<i32>, has_thumbnail: bool) -> Self {
        match (page_count, has_thumbnail) {
            (Some(_), true) => Self::Complete,
            (Some(_), false) => Self::CoverPending,
            (None, _) => Self::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_statuses_round_trip_through_the_database_value() {
        for status in AnalysisStatus::ALL {
            assert_eq!(AnalysisStatus::from_db(status.as_str()), Some(status));
        }
        assert_eq!(AnalysisStatus::from_db("unknown"), None);
    }

    #[test]
    fn only_discovered_and_analyzing_count_as_pending() {
        for status in AnalysisStatus::ALL {
            assert_eq!(
                status.is_pending(),
                matches!(
                    status,
                    AnalysisStatus::Discovered | AnalysisStatus::Analyzing
                )
            );
        }
    }

    #[test]
    fn failed_books_are_retried_but_not_counted_as_pending() {
        assert!(AnalysisStatus::Failed.is_analysis_candidate());
        assert!(!AnalysisStatus::Failed.is_pending());
        assert!(!AnalysisStatus::Failed.is_thumbnail_candidate());
    }

    #[test]
    fn cover_pending_is_claimable_by_thumbnail_jobs_only() {
        assert!(AnalysisStatus::CoverPending.is_thumbnail_candidate());
        assert!(!AnalysisStatus::CoverPending.is_analysis_candidate());
        assert!(!AnalysisStatus::CoverPending.is_pending());
    }

    #[test]
    fn analyzing_stays_claimable_so_crashed_jobs_recover() {
        assert!(AnalysisStatus::Analyzing.is_analysis_candidate());
        assert!(AnalysisStatus::Analyzing.is_pending());
    }

    #[test]
    fn extraction_transitions_follow_the_cover_availability() {
        assert_eq!(
            AnalysisStatus::after_extraction(Some(12), true),
            AnalysisStatus::Complete
        );
        assert_eq!(
            AnalysisStatus::after_extraction(Some(12), false),
            AnalysisStatus::CoverPending
        );
        assert_eq!(
            AnalysisStatus::after_extraction(None, false),
            AnalysisStatus::Failed
        );
        assert_eq!(
            AnalysisStatus::after_extraction(None, true),
            AnalysisStatus::Failed
        );
    }
}
