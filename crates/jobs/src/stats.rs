//! Shape of `index_jobs.stats_json` for the indexer jobs.
//!
//! The indexer writes this value while a job runs and when it finishes; the
//! notification module and the admin UI read it. The field names are therefore
//! part of the `stats_json` contract: the backoffice reads them by name, so
//! renaming one is a visible change. Unknown fields from an older or newer
//! writer are ignored and missing ones default to `0` / empty.

use serde::{Deserialize, Serialize};

/// What a scan (or a single-unit job) reports through `index_jobs.stats_json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub removed_files: usize,
    pub errors: usize,
    /// Non-fatal problems (partial walk, non-UTF-8 paths, skipped subtrees).
    pub warnings: usize,
    pub new_series: usize,
    pub new_series_names: Vec<String>,
    pub new_book_titles: Vec<String>,
}
