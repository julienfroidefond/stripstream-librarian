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

// ---------------------------------------------------------------------------
// End-to-end provider tests
//
// These tests hit real external APIs — run them explicitly with:
//   cargo test -p api providers_e2e -- --ignored --nocapture
// ---------------------------------------------------------------------------

#[cfg(test)]
mod providers_e2e {
    use super::*;

    fn config_fr() -> ProviderConfig {
        ProviderConfig { api_key: None, language: "fr".to_string() }
    }

    fn config_en() -> ProviderConfig {
        ProviderConfig { api_key: None, language: "en".to_string() }
    }

    fn print_candidate(name: &str, c: &SeriesCandidate) {
        println!("\n=== {name} — best candidate ===");
        println!("  title:         {:?}", c.title);
        println!("  external_id:   {:?}", c.external_id);
        println!("  authors:       {:?}", c.authors);
        println!("  description:   {:?}", c.description.as_deref().map(|d| &d[..d.len().min(120)]));
        println!("  publishers:    {:?}", c.publishers);
        println!("  start_year:    {:?}", c.start_year);
        println!("  total_volumes: {:?}", c.total_volumes);
        println!("  cover_url:     {}", c.cover_url.is_some());
        println!("  external_url:  {}", c.external_url.is_some());
        println!("  confidence:    {:.2}", c.confidence);
        println!("  metadata_json: {}", serde_json::to_string_pretty(&c.metadata_json).unwrap_or_default());
    }

    fn print_books(name: &str, books: &[BookCandidate]) {
        println!("\n=== {name} — {} books ===", books.len());
        for (i, b) in books.iter().take(5).enumerate() {
            println!(
                "  [{}] vol={:?} title={:?} authors={} isbn={:?} pages={:?} lang={:?} date={:?} cover={}",
                i, b.volume_number, b.title, b.authors.len(), b.isbn, b.page_count, b.language, b.publish_date, b.cover_url.is_some()
            );
        }
        if books.len() > 5 { println!("  ... and {} more", books.len() - 5); }

        let with_vol = books.iter().filter(|b| b.volume_number.is_some()).count();
        let with_isbn = books.iter().filter(|b| b.isbn.is_some()).count();
        let with_authors = books.iter().filter(|b| !b.authors.is_empty()).count();
        let with_date = books.iter().filter(|b| b.publish_date.is_some()).count();
        let with_cover = books.iter().filter(|b| b.cover_url.is_some()).count();
        let with_pages = books.iter().filter(|b| b.page_count.is_some()).count();
        println!("  --- field coverage ---");
        println!("    volume_number: {with_vol}/{}", books.len());
        println!("    isbn:          {with_isbn}/{}", books.len());
        println!("    authors:       {with_authors}/{}", books.len());
        println!("    publish_date:  {with_date}/{}", books.len());
        println!("    cover_url:     {with_cover}/{}", books.len());
        println!("    page_count:    {with_pages}/{}", books.len());
    }

    // --- Google Books ---

    #[tokio::test]
    #[ignore]
    async fn google_books_search_and_books() {
        let p = get_provider("google_books").unwrap();
        let cfg = config_en();

        let candidates = p.search_series("Blacksad", &cfg).await.unwrap();
        assert!(!candidates.is_empty(), "google_books: no results for Blacksad");
        print_candidate("google_books", &candidates[0]);

        let books = p.get_series_books(&candidates[0].external_id, &cfg).await.unwrap();
        print_books("google_books", &books);
        assert!(!books.is_empty(), "google_books: no books returned");
    }

    // --- Open Library ---

    #[tokio::test]
    #[ignore]
    async fn open_library_search_and_books() {
        let p = get_provider("open_library").unwrap();
        let cfg = config_en();

        let candidates = p.search_series("Sandman Neil Gaiman", &cfg).await.unwrap();
        assert!(!candidates.is_empty(), "open_library: no results for Sandman");
        print_candidate("open_library", &candidates[0]);

        let books = p.get_series_books(&candidates[0].external_id, &cfg).await.unwrap();
        print_books("open_library", &books);
        assert!(!books.is_empty(), "open_library: no books returned");
    }

    // --- AniList ---

    #[tokio::test]
    #[ignore]
    async fn anilist_search_finished() {
        let p = get_provider("anilist").unwrap();
        let cfg = config_fr();

        let candidates = p.search_series("Death Note", &cfg).await.unwrap();
        assert!(!candidates.is_empty(), "anilist: no results for Death Note");
        print_candidate("anilist (finished)", &candidates[0]);

        let best = &candidates[0];
        assert!(best.total_volumes.is_some(), "anilist: finished series should have total_volumes");
        assert!(best.description.is_some(), "anilist: should have description");
        assert!(!best.authors.is_empty(), "anilist: should have authors");

        let status = best.metadata_json.get("status").and_then(|s| s.as_str());
        assert_eq!(status, Some("FINISHED"), "anilist: Death Note should be FINISHED");

        let books = p.get_series_books(&best.external_id, &cfg).await.unwrap();
        print_books("anilist (Death Note)", &books);
        assert!(books.len() >= 12, "anilist: Death Note should have ≥12 volumes, got {}", books.len());
    }

    #[tokio::test]
    #[ignore]
    async fn anilist_search_ongoing() {
        let p = get_provider("anilist").unwrap();
        let cfg = config_fr();

        let candidates = p.search_series("One Piece", &cfg).await.unwrap();
        assert!(!candidates.is_empty(), "anilist: no results for One Piece");
        print_candidate("anilist (ongoing)", &candidates[0]);

        let best = &candidates[0];
        let status = best.metadata_json.get("status").and_then(|s| s.as_str());
        assert_eq!(status, Some("RELEASING"), "anilist: One Piece should be RELEASING");

        let volume_source = best.metadata_json.get("volume_source").and_then(|s| s.as_str());
        println!("  volume_source: {:?}", volume_source);
        println!("  total_volumes: {:?}", best.total_volumes);
    }

    // --- Bédéthèque ---

    #[tokio::test]
    #[ignore]
    async fn bedetheque_search_and_books() {
        let p = get_provider("bedetheque").unwrap();
        let cfg = config_fr();

        let candidates = p.search_series("De Cape et de Crocs", &cfg).await.unwrap();
        assert!(!candidates.is_empty(), "bedetheque: no results");
        print_candidate("bedetheque", &candidates[0]);

        let best = &candidates[0];
        assert!(best.description.is_some(), "bedetheque: should have description");
        assert!(!best.authors.is_empty(), "bedetheque: should have authors");
        assert!(!best.publishers.is_empty(), "bedetheque: should have publishers");
        assert!(best.start_year.is_some(), "bedetheque: should have start_year");
        assert!(best.total_volumes.is_some(), "bedetheque: should have total_volumes");

        // Enriched metadata_json
        let mj = &best.metadata_json;
        assert!(mj.get("genres").and_then(|g| g.as_array()).map(|a| !a.is_empty()).unwrap_or(false), "bedetheque: should have genres");
        assert!(mj.get("status").and_then(|s| s.as_str()).is_some(), "bedetheque: should have status");

        let books = p.get_series_books(&best.external_id, &cfg).await.unwrap();
        print_books("bedetheque", &books);
        assert!(books.len() >= 12, "bedetheque: De Cape et de Crocs should have ≥12 volumes, got {}", books.len());
    }

    // --- ComicVine (needs API key) ---

    #[tokio::test]
    #[ignore]
    async fn comicvine_no_key() {
        let p = get_provider("comicvine").unwrap();
        let cfg = config_en();

        let result = p.search_series("Batman", &cfg).await;
        println!("\n=== comicvine (no key) ===");
        match result {
            Ok(c) => println!("  returned {} candidates (unexpected without key)", c.len()),
            Err(e) => println!("  expected error: {e}"),
        }
    }

    // --- Cross-provider comparison ---

    #[tokio::test]
    #[ignore]
    async fn cross_provider_blacksad() {
        println!("\n{}", "=".repeat(60));
        println!("  Cross-provider comparison: Blacksad");
        println!("{}\n", "=".repeat(60));

        let providers: Vec<(&str, ProviderConfig)> = vec![
            ("google_books", config_en()),
            ("open_library", config_en()),
            ("anilist", config_fr()),
            ("bedetheque", config_fr()),
        ];

        for (name, cfg) in &providers {
            let p = get_provider(name).unwrap();
            match p.search_series("Blacksad", cfg).await {
                Ok(candidates) if !candidates.is_empty() => {
                    let b = &candidates[0];
                    println!("[{name}]  title={:?}  authors={}  desc={}  pubs={}  year={:?}  vols={:?}  cover={}  url={}  conf={:.2}",
                        b.title, b.authors.len(), b.description.is_some(), b.publishers.len(),
                        b.start_year, b.total_volumes, b.cover_url.is_some(), b.external_url.is_some(), b.confidence);
                }
                Ok(_) => println!("[{name}]  no results"),
                Err(e) => println!("[{name}]  error: {e}"),
            }
        }
    }
}
