pub mod anilist;
pub mod bdphile;
pub mod bdtheque;
pub mod comicvine;
pub mod google_books;
pub mod open_library;
pub mod run;
pub(crate) mod scoring;
mod scraping;
pub mod senscritique;
pub mod series_metadata;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use run::{ProviderBlock, ProviderCall, ProviderError, ProviderRun};

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

/// Discovery sources backed by the SensCritique metadata provider.
///
/// The discovery module exposes several ranking lists (`sc_trending_bd`,
/// `sc_best_manga`, `senscritique_bd`, …) that are *discovery sources*, not
/// metadata providers. They all resolve to the same [`senscritique`] provider,
/// so their ids must never be stored in `external_metadata_links.provider` nor
/// passed to [`get_provider`] — that yields an "unknown provider" error on the
/// next metadata refresh.
const SENSCRITIQUE_DISCOVERY_SOURCES: &[&str] = &[
    "senscritique",
    "senscritique_bd",
    "sc_trending_bd",
    "sc_trending_manga",
    "sc_best_bd",
    "sc_best_manga",
];

/// Map a discovery source id to the metadata provider that backs it.
///
/// Ids that are already a provider name are returned unchanged:
/// `senscritique_bd` → `senscritique`, `sc_best_manga` → `senscritique`,
/// `anilist` → `anilist`.
pub fn canonical_provider_name(name: &str) -> &str {
    if SENSCRITIQUE_DISCOVERY_SOURCES.contains(&name) {
        "senscritique"
    } else {
        name
    }
}

/// Display label of a provider id (`senscritique` → `SensCritique`).
///
/// Falls back to the id itself for a provider that is not in the list, so a
/// message never loses the name it was built from.
pub fn provider_label(name: &str) -> String {
    available_providers()
        .into_iter()
        .find(|descriptor| descriptor.id == name)
        .map(|descriptor| descriptor.label)
        .unwrap_or_else(|| name.to_string())
}

/// Resolve `name` to an implemented metadata provider.
///
/// Discovery source ids are canonicalized first (`senscritique_bd` →
/// `senscritique`). On failure the *canonical* name is returned so callers can
/// report exactly what was rejected — the old code silently fell back to Google
/// Books, which surfaced a confusing error and hid the real one (#112).
pub fn resolve_provider(name: &str) -> Result<Box<dyn MetadataProvider>, String> {
    resolve_provider_with(name, UnknownProvider::Reject)
}

/// Provider used when a legacy link names something that is not a provider.
pub const FALLBACK_PROVIDER: &str = "google_books";

/// What to do when a name matches no metadata provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnknownProvider {
    /// Report the canonical name back (search, batch, refresh).
    Reject,
    /// Use [`FALLBACK_PROVIDER`] (links created outside the metadata providers).
    Fallback,
}

/// The one resolver: canonicalize, then apply the caller's unknown-name policy.
fn resolve_provider_with(
    name: &str,
    on_unknown: UnknownProvider,
) -> Result<Box<dyn MetadataProvider>, String> {
    let canonical = canonical_provider_name(name);
    match (get_provider(canonical), on_unknown) {
        (Some(provider), _) => Ok(provider),
        (None, UnknownProvider::Fallback) => {
            get_provider(FALLBACK_PROVIDER).ok_or_else(|| canonical.to_string())
        }
        (None, UnknownProvider::Reject) => Err(canonical.to_string()),
    }
}

/// Resolve the provider behind a stored metadata link (#112).
///
/// Canonicalizes discovery source ids like [`resolve_provider`], but keeps the
/// historic Google Books fallback for a link whose provider is not a metadata
/// provider (e.g. a Prowlarr download). This is the explicit fallback policy,
/// not a silent default: search, batch and refresh stay strict.
pub fn resolve_link_provider(name: &str) -> Result<Box<dyn MetadataProvider>, String> {
    resolve_provider_with(name, UnknownProvider::Fallback)
}

// ---------------------------------------------------------------------------
// Shared provider helpers
// ---------------------------------------------------------------------------

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
    /// Match score against the searched name (see [`scoring`]).
    pub score: scoring::Score,
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

/// How a provider's books are matched against the local books of a series.
///
/// This is a provider capability, not a caller decision: the HTML scrapers
/// expose a oneshot flag and must be allowed to match an unnumbered book,
/// while the historic API providers keep the regular/integral matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingPolicy {
    /// Volume number, then title containment; oneshots never match.
    Standard,
    /// Same matching, but an unnumbered book flagged `oneshot` can match.
    HtmlScraper,
}

impl MatchingPolicy {
    /// Whether an unnumbered book flagged `oneshot` may match a local book.
    pub fn allows_oneshots(&self) -> bool {
        matches!(self, Self::HtmlScraper)
    }
}

/// Trait that all metadata providers must implement
#[async_trait::async_trait]
pub trait MetadataProvider: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// How this provider's books are matched against local books.
    ///
    /// Defaults to [`MatchingPolicy::Standard`]; only the HTML scrapers that
    /// flag oneshots override it.
    fn matching_policy(&self) -> MatchingPolicy {
        MatchingPolicy::Standard
    }

    async fn search_series(
        &self,
        query: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<SeriesCandidate>, ProviderError>;

    /// Fetch the exact series previously linked to this provider.
    ///
    /// Refreshes must use this method rather than a name search: search rankings
    /// are not stable enough to safely update an approved link.
    async fn get_series(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> Result<SeriesCandidate, ProviderError>;

    async fn get_series_books(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> Result<Vec<BookCandidate>, ProviderError>;
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

    /// Regression #112: every discovery source must resolve to a real provider.
    #[test]
    fn canonical_provider_name_normalizes_every_discovery_source() {
        for source in [
            "senscritique",
            "senscritique_bd",
            "sc_trending_bd",
            "sc_trending_manga",
            "sc_best_bd",
            "sc_best_manga",
        ] {
            assert_eq!(
                canonical_provider_name(source),
                "senscritique",
                "{source} must be normalized to senscritique"
            );
        }
    }

    #[test]
    fn canonical_provider_name_keeps_real_providers_untouched() {
        for provider in [
            "google_books",
            "open_library",
            "comicvine",
            "anilist",
            "bdphile",
            "bdtheque",
            "prowlarr",
        ] {
            assert_eq!(canonical_provider_name(provider), provider);
        }
    }

    /// Regression #112: a link created from any discovery list must resolve to a
    /// real provider, no matter which SensCritique-backed source id it carries.
    #[test]
    fn resolve_provider_accepts_every_discovery_source() {
        for source in [
            "senscritique",
            "senscritique_bd",
            "sc_trending_bd",
            "sc_trending_manga",
            "sc_best_bd",
            "sc_best_manga",
        ] {
            let provider = resolve_provider(source)
                .unwrap_or_else(|_| panic!("{source} must resolve to a provider"));
            assert_eq!(
                provider.name(),
                "senscritique",
                "{source} must resolve to the senscritique provider"
            );
        }
    }

    #[test]
    fn resolve_provider_accepts_real_providers() {
        for provider in [
            "google_books",
            "open_library",
            "comicvine",
            "anilist",
            "bdphile",
            "bdtheque",
            "senscritique",
        ] {
            assert_eq!(
                resolve_provider(provider).unwrap().name(),
                provider,
                "{provider} must resolve to itself"
            );
        }
    }

    #[test]
    fn resolve_provider_rejects_unknown_names() {
        // `prowlarr` is not a metadata provider: it is a download source, so the
        // refresh/search paths must reject it rather than fall back to Google
        // Books.
        for unknown in ["prowlarr", "senscritique_bd_typo", "", "google"] {
            let err = resolve_provider(unknown)
                .err()
                .unwrap_or_else(|| panic!("{unknown} must not resolve"));
            assert_eq!(err, unknown, "the rejected name must be reported verbatim");
        }
    }

    /// The provider list exposed to the UI must be exactly the set of providers
    /// that `get_provider` implements: a descriptor without an implementation
    /// would show up in the backoffice and then fail with a 400 at search or
    /// refresh time. Guards against the two lists drifting apart.
    #[test]
    fn available_providers_are_all_implemented_and_resolvable() {
        let descriptors = available_providers();
        assert!(
            !descriptors.is_empty(),
            "the provider list must not be empty"
        );

        let mut seen = std::collections::HashSet::new();
        for d in &descriptors {
            assert!(seen.insert(d.id.clone()), "duplicate provider id: {}", d.id);

            let provider = get_provider(&d.id)
                .unwrap_or_else(|| panic!("{} is listed but not implemented", d.id));
            assert_eq!(provider.name(), d.id.as_str());

            assert_eq!(
                resolve_provider(&d.id)
                    .unwrap_or_else(|_| panic!("{} is listed but does not resolve", d.id))
                    .name(),
                d.id.as_str()
            );
        }
    }

    /// The matching policy is a provider capability, not a caller decision.
    #[test]
    fn each_provider_declares_its_matching_policy() {
        for id in [
            "google_books",
            "open_library",
            "comicvine",
            "anilist",
            "senscritique",
        ] {
            assert_eq!(
                get_provider(id).unwrap().matching_policy(),
                MatchingPolicy::Standard,
                "{id} keeps the standard matching"
            );
        }
        for id in ["bdtheque", "bdphile"] {
            assert_eq!(
                get_provider(id).unwrap().matching_policy(),
                MatchingPolicy::HtmlScraper,
                "{id} exposes oneshot books"
            );
        }
    }

    #[test]
    fn only_the_html_scraper_policy_allows_oneshots() {
        assert!(MatchingPolicy::HtmlScraper.allows_oneshots());
        assert!(!MatchingPolicy::Standard.allows_oneshots());
    }

    /// The legacy link fallback is an explicit, named policy: strict callers
    /// reject an unknown name, the link path uses the one declared fallback.
    #[test]
    fn the_link_fallback_is_explicit_and_named() {
        assert_eq!(FALLBACK_PROVIDER, "google_books");
        assert_eq!(
            resolve_provider("prowlarr")
                .err()
                .expect("strict callers reject an unknown name"),
            "prowlarr"
        );

        for unknown in ["prowlarr", "senscritique_bd_typo", ""] {
            assert_eq!(
                resolve_link_provider(unknown).unwrap().name(),
                FALLBACK_PROVIDER,
                "{unknown} must use the declared fallback"
            );
        }
    }

    #[test]
    fn the_link_fallback_still_resolves_real_providers() {
        assert_eq!(resolve_link_provider("anilist").unwrap().name(), "anilist");
        assert_eq!(
            resolve_link_provider("senscritique_bd").unwrap().name(),
            "senscritique",
            "a discovery alias is canonicalized, never sent to the fallback"
        );
    }

    /// Fake adapter with a fixed answer, to drive the run policy.
    struct StubProvider {
        outcome: Result<Vec<SeriesCandidate>, ProviderError>,
    }

    #[async_trait::async_trait]
    impl MetadataProvider for StubProvider {
        fn name(&self) -> &str {
            "senscritique"
        }

        async fn search_series(
            &self,
            _query: &str,
            _config: &ProviderConfig,
        ) -> Result<Vec<SeriesCandidate>, ProviderError> {
            self.outcome.clone()
        }

        async fn get_series(
            &self,
            _external_id: &str,
            _config: &ProviderConfig,
        ) -> Result<SeriesCandidate, ProviderError> {
            Err(ProviderError::Fatal("not implemented".to_string()))
        }

        async fn get_series_books(
            &self,
            _external_id: &str,
            _config: &ProviderConfig,
        ) -> Result<Vec<BookCandidate>, ProviderError> {
            Ok(vec![])
        }
    }

    /// Each adapter error variant is answered as-is; only a rate limit blocks.
    #[tokio::test]
    async fn every_provider_error_variant_drives_the_run_policy() {
        let config = ProviderConfig::default();

        let provider = StubProvider {
            outcome: Err(ProviderError::RateLimited("429".to_string())),
        };
        let mut run = ProviderRun::new();
        match run
            .call("senscritique", || provider.search_series("q", &config))
            .await
        {
            ProviderCall::Answered(Err(error)) => assert!(error.is_rate_limited()),
            _ => panic!("a rate limit is answered, not skipped"),
        }
        assert!(run.is_blocked("senscritique"));

        let provider = StubProvider {
            outcome: Err(ProviderError::transient("timeout")),
        };
        let mut run = ProviderRun::new();
        match run
            .call("senscritique", || provider.search_series("q", &config))
            .await
        {
            ProviderCall::Answered(Err(error)) => assert!(!error.is_rate_limited()),
            _ => panic!("a transient error is answered"),
        }
        assert!(!run.is_blocked("senscritique"));

        let provider = StubProvider {
            outcome: Err(ProviderError::Fatal("bad id".to_string())),
        };
        let mut run = ProviderRun::new();
        match run
            .call("senscritique", || provider.search_series("q", &config))
            .await
        {
            ProviderCall::Answered(Err(error)) => assert!(!error.is_rate_limited()),
            _ => panic!("a fatal error is answered"),
        }
        assert!(!run.is_blocked("senscritique"));
    }
}
