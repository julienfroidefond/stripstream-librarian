pub mod anilist;
pub mod bedetheque;
pub mod comicvine;
pub mod google_books;
pub mod open_library;

use serde::{Deserialize, Serialize};

/// Configuration passed to providers (API keys, etc.)
#[derive(Debug, Clone, Default)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    /// Preferred language for metadata results (ISO 639-1: "en", "fr", "es"). Defaults to "en".
    pub language: String,
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
pub trait MetadataProvider: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;

    fn search_series(
        &self,
        query: &str,
        config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<SeriesCandidate>, String>> + Send + '_>,
    >;

    fn get_series_books(
        &self,
        external_id: &str,
        config: &ProviderConfig,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<BookCandidate>, String>> + Send + '_>,
    >;
}

/// Factory function to get a provider by name
pub fn get_provider(name: &str) -> Option<Box<dyn MetadataProvider>> {
    match name {
        "google_books" => Some(Box::new(google_books::GoogleBooksProvider)),
        "open_library" => Some(Box::new(open_library::OpenLibraryProvider)),
        "comicvine" => Some(Box::new(comicvine::ComicVineProvider)),
        "anilist" => Some(Box::new(anilist::AniListProvider)),
        "bedetheque" => Some(Box::new(bedetheque::BedethequeProvider)),
        _ => None,
    }
}
