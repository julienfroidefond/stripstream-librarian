pub mod anilist;
pub mod bdphile;
pub mod bdtheque;
pub mod comicvine;
pub mod google_books;
pub mod open_library;
mod scraping;
pub mod senscritique;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Configuration passed to providers (API keys, etc.)
#[derive(Debug, Clone, Default)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    /// Preferred language for metadata results (ISO 639-1: "en", "fr", "es"). Defaults to "en".
    pub language: String,
    /// When true, return detailed results (e.g., per-edition for SensCritique).
    /// Set to false in batch mode to reduce API calls.
    pub detailed: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProviderDescriptor {
    pub id: String,
    pub label: String,
    pub requires_api_key: bool,
}

pub fn available_providers() -> Vec<ProviderDescriptor> {
    [
        ("google_books", "Google Books", true),
        ("open_library", "Open Library", false),
        ("comicvine", "ComicVine", true),
        ("anilist", "AniList", false),
        ("bdtheque", "BDTheque", false),
        ("bdphile", "BDphile", false),
        ("senscritique", "SensCritique", false),
    ]
    .into_iter()
    .map(|(id, label, requires_api_key)| ProviderDescriptor {
        id: id.to_string(),
        label: label.to_string(),
        requires_api_key,
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Shared provider helpers
// ---------------------------------------------------------------------------

/// Confidence score between a candidate `title` and the original `query`.
///
/// Exact match → `1.0`, prefix match in either direction → `0.8`, substring
/// match in either direction → `0.7`, otherwise a character-overlap ratio
/// clamped to `0.1..=0.6`.
pub(crate) fn compute_confidence(title: &str, query: &str) -> f32 {
    let title_lower = title.to_lowercase();
    if title_lower == query {
        1.0
    } else if title_lower.starts_with(query) || query.starts_with(&title_lower) {
        0.8
    } else if title_lower.contains(query) || query.contains(&title_lower) {
        0.7
    } else {
        let common: usize = query.chars().filter(|c| title_lower.contains(*c)).count();
        let max_len = query.len().max(title_lower.len()).max(1);
        (common as f32 / max_len as f32).clamp(0.1, 0.6)
    }
}

/// Percent-encode `s` for use in a URL query string.
///
/// Keeps the RFC 3986 unreserved characters (`A-Z a-z 0-9 - _ . ~`) and encodes
/// every other byte as `%XX`.
pub(crate) fn urlencoded(s: &str) -> String {
    let mut result = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => result.push_str(&format!("%{:02X}", byte)),
        }
    }
    result
}

/// Strip HTML tags from `s` and trim the surrounding whitespace.
pub(crate) fn strip_html(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

/// A candidate series returned by a provider search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesCandidate {
    pub external_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub publishers: Vec<String>,
    pub start_year: Option<i32>,
    pub total_volumes: Option<i32>,
    pub cover_url: Option<String>,
    pub external_url: Option<String>,
    pub confidence: f32,
    pub metadata_json: serde_json::Value,
}

/// A candidate book within a series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookCandidate {
    pub external_book_id: String,
    pub title: String,
    pub volume_number: Option<i32>,
    pub authors: Vec<String>,
    pub isbn: Option<String>,
    pub summary: Option<String>,
    pub cover_url: Option<String>,
    pub page_count: Option<i32>,
    pub language: Option<String>,
    pub publish_date: Option<String>,
    pub metadata_json: serde_json::Value,
}

/// Trait that all metadata providers must implement
#[async_trait::async_trait]
pub trait MetadataProvider: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;

    async fn search_series(
        &self,
        query: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<SeriesCandidate>, String>;

    /// Fetch the exact series previously linked to this provider.
    ///
    /// Refreshes must use this method rather than a name search: search rankings
    /// are not stable enough to safely update an approved link.
    async fn get_series(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> Result<SeriesCandidate, String>;

    async fn get_series_books(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<BookCandidate>, String>;
}

/// Factory function to get a provider by name
pub fn get_provider(name: &str) -> Option<Box<dyn MetadataProvider>> {
    match name {
        "google_books" => Some(Box::new(google_books::GoogleBooksProvider)),
        "open_library" => Some(Box::new(open_library::OpenLibraryProvider)),
        "comicvine" => Some(Box::new(comicvine::ComicVineProvider)),
        "anilist" => Some(Box::new(anilist::AniListProvider)),
        "bdphile" => Some(Box::new(bdphile::BdphileProvider)),
        "bdtheque" => Some(Box::new(bdtheque::BdthequeProvider)),
        "senscritique" => Some(Box::new(senscritique::SensCritiqueProvider)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// End-to-end provider tests
//
// These tests hit real external APIs — run them explicitly with:
//   cargo test -p api providers_e2e -- --ignored --nocapture
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "tests/providers_e2e.rs"]
mod providers_e2e;

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn confidence_exact_match() {
        assert_eq!(compute_confidence("Naruto", "naruto"), 1.0);
    }

    #[test]
    fn confidence_prefix_match() {
        assert_eq!(compute_confidence("Naruto Shippuden", "naruto"), 0.8);
        assert_eq!(compute_confidence("Naruto", "naruto shippuden"), 0.8);
    }

    #[test]
    fn confidence_contains_match() {
        assert_eq!(compute_confidence("The Art of Naruto", "naruto"), 0.7);
        assert_eq!(compute_confidence("naruto", "the art of naruto"), 0.7);
    }

    #[test]
    fn confidence_overlap_is_clamped() {
        let score = compute_confidence("abcdef", "azbycxdw");
        assert!((0.1..=0.6).contains(&score), "score out of range: {score}");
        // No common character → floor of 0.1.
        assert_eq!(compute_confidence("xyz", "abc"), 0.1);
    }

    #[test]
    fn urlencoded_keeps_unreserved_and_encodes_rest() {
        assert_eq!(urlencoded("abcXYZ019-_.~"), "abcXYZ019-_.~");
        assert_eq!(urlencoded("a b"), "a%20b");
        assert_eq!(urlencoded("café"), "caf%C3%A9");
        assert_eq!(urlencoded("a/b?c=d&e"), "a%2Fb%3Fc%3Dd%26e");
    }

    #[test]
    fn strip_html_removes_tags_and_trims() {
        assert_eq!(strip_html("<p>hello <b>world</b></p>"), "hello world");
        assert_eq!(strip_html("  plain text  "), "plain text");
        assert_eq!(strip_html("<div><span></span></div>"), "");
    }
}
