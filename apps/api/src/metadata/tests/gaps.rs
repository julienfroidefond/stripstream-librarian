//! Integration tests for metadata-gap detection.
//!
//! Covers `get_gap_summary` plus the `gap=` filters on the series and books list
//! endpoints. Handlers are called directly against an ephemeral Postgres from
//! `#[sqlx::test]`, so the real aggregate / array SQL is exercised without an HTTP server.

use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};

use axum::extract::{Query, State};
use axum::Json;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock, Semaphore};
use uuid::Uuid;

use super::*;
use crate::books::{list_books, ListBooksQuery};
use crate::series::{list_all_series, ListAllSeriesQuery};
use crate::state::{
    DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};
use crate::AppState;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn test_state(pool: PgPool) -> AppState {
    AppState {
        pool,
        bootstrap_token: Arc::from("test-token"),
        page_cache: Arc::new(Mutex::new(crate::state::PageCache::new(1))),
        disk_cache_stats: Arc::new(Mutex::new(None::<DiskCacheStatsSnapshot>)),
        page_render_locks: Arc::new(PageRenderLocks::new(1)),
        page_render_limit: Arc::new(Semaphore::new(1)),
        metrics: Arc::new(Metrics {
            requests_total: AtomicU64::new(0),
            page_cache_hits: AtomicU64::new(0),
            page_cache_misses: AtomicU64::new(0),
        }),
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit::new())),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(HashMap::new())),
    }
}

async fn create_library(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO libraries (id, name, root_path) \
         VALUES (gen_random_uuid(), $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(format!("/libraries/{name}-{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A fully-populated series unless a field is overridden. Each `None` / empty
/// override maps to exactly one series gap.
struct SeriesSeed<'a> {
    name: &'a str,
    genres: &'a [&'a str],
    authors: &'a [&'a str],
    publishers: &'a [&'a str],
    description: Option<&'a str>,
    start_year: Option<i32>,
    cover_url: Option<&'a str>,
}

impl Default for SeriesSeed<'_> {
    fn default() -> Self {
        Self {
            name: "Series",
            genres: &["Action"],
            authors: &["Author"],
            publishers: &["Publisher"],
            description: Some("Description"),
            start_year: Some(2000),
            cover_url: Some("/cover.webp"),
        }
    }
}

async fn insert_series(pool: &PgPool, library_id: Uuid, seed: SeriesSeed<'_>) -> Uuid {
    let genres: Vec<String> = seed.genres.iter().map(|g| g.to_string()).collect();
    let authors: Vec<String> = seed.authors.iter().map(|a| a.to_string()).collect();
    let publishers: Vec<String> = seed.publishers.iter().map(|p| p.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO series \
             (id, library_id, name, genres, authors, publishers, description, start_year, cover_url) \
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(library_id)
    .bind(seed.name)
    .bind(genres)
    .bind(authors)
    .bind(publishers)
    .bind(seed.description)
    .bind(seed.start_year)
    .bind(seed.cover_url)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A fully-populated book unless a field is overridden. Each `None` / empty
/// override maps to exactly one book gap.
struct BookSeed<'a> {
    title: &'a str,
    series_id: Option<Uuid>,
    volume: Option<i32>,
    summary: Option<&'a str>,
    isbn: Option<&'a str>,
    thumbnail_path: Option<&'a str>,
    authors: &'a [&'a str],
    author: Option<&'a str>,
    publish_date: Option<&'a str>,
    language: Option<&'a str>,
}

impl Default for BookSeed<'_> {
    fn default() -> Self {
        Self {
            title: "Book",
            series_id: None,
            volume: None,
            summary: Some("Summary"),
            isbn: Some("978"),
            thumbnail_path: Some("/thumb.webp"),
            authors: &["Author"],
            author: Some("Author"),
            publish_date: Some("2020"),
            language: Some("fr"),
        }
    }
}

async fn insert_book(pool: &PgPool, library_id: Uuid, seed: BookSeed<'_>) -> Uuid {
    let authors: Vec<String> = seed.authors.iter().map(|a| a.to_string()).collect();
    sqlx::query_scalar(
        "INSERT INTO books \
             (id, library_id, kind, title, series_id, volume, volume_type, \
              summary, isbn, thumbnail_path, authors, author, publish_date, language) \
         VALUES (gen_random_uuid(), $1, 'comic', $2, $3, $4, 'regular', \
                 $5, $6, $7, $8, $9, $10, $11) \
         RETURNING id",
    )
    .bind(library_id)
    .bind(seed.title)
    .bind(seed.series_id)
    .bind(seed.volume)
    .bind(seed.summary)
    .bind(seed.isbn)
    .bind(seed.thumbnail_path)
    .bind(authors)
    .bind(seed.author)
    .bind(seed.publish_date)
    .bind(seed.language)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Insert an approved provider link for a series, optionally carrying a rating.
async fn insert_approved_link(
    pool: &PgPool,
    library_id: Uuid,
    series_id: Uuid,
    provider_rating: Option<f32>,
    provider_rating_scale: Option<f32>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO external_metadata_links \
             (library_id, series_id, provider, external_id, status, provider_rating, provider_rating_scale) \
         VALUES ($1, $2, 'test', $3, 'approved', $4, $5) RETURNING id",
    )
    .bind(library_id)
    .bind(series_id)
    .bind(format!("ext:{}", Uuid::new_v4()))
    .bind(provider_rating)
    .bind(provider_rating_scale)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn gap_query(library_id: Option<Uuid>) -> Query<GapSummaryQuery> {
    Query(GapSummaryQuery { library_id })
}

fn series_gap_query(library_id: Option<Uuid>, gap: &str) -> Query<ListAllSeriesQuery> {
    Query(ListAllSeriesQuery {
        q: None,
        library_id,
        reading_status: None,
        series_status: None,
        has_missing: None,
        metadata_provider: None,
        gap: Some(gap.to_string()),
        author: None,
        page: None,
        limit: None,
        has_books: None,
        no_books: None,
        sort: None,
        genre: None,
        volume_type: None,
        rated_only: None,
    })
}

fn books_gap_query(library_id: Option<Uuid>, gap: &str) -> Query<ListBooksQuery> {
    Query(ListBooksQuery {
        q: None,
        library_id,
        kind: None,
        format: None,
        series: None,
        reading_status: None,
        author: None,
        page: None,
        limit: None,
        sort: None,
        metadata_provider: None,
        gap: Some(gap.to_string()),
    })
}

async fn summary(state: &AppState, library_id: Option<Uuid>) -> GapSummary {
    let Json(summary) = get_gap_summary(State(state.clone()), gap_query(library_id))
        .await
        .unwrap();
    summary
}

async fn series_ids(state: &AppState, library_id: Uuid, gap: &str) -> Vec<Uuid> {
    let Json(page) = list_all_series(
        State(state.clone()),
        None,
        series_gap_query(Some(library_id), gap),
    )
    .await
    .unwrap();
    page.items.iter().map(|s| s.series_id).collect()
}

async fn book_ids(state: &AppState, library_id: Uuid, gap: &str) -> Vec<Uuid> {
    let Json(page) = list_books(
        State(state.clone()),
        books_gap_query(Some(library_id), gap),
        None,
    )
    .await
    .unwrap();
    page.items.iter().map(|b| b.id).collect()
}

fn sorted(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort();
    ids
}

// ---------------------------------------------------------------------------
// gap summary
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn gap_summary_counts_every_dimension(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "main").await;

    let complete = insert_series(&pool, library, SeriesSeed::default()).await;
    insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "Empty",
            genres: &[],
            authors: &[],
            publishers: &[],
            description: None,
            start_year: None,
            cover_url: None,
        },
    )
    .await;

    insert_book(
        &pool,
        library,
        BookSeed {
            title: "Complete",
            series_id: Some(complete),
            volume: Some(1),
            ..BookSeed::default()
        },
    )
    .await;
    insert_book(
        &pool,
        library,
        BookSeed {
            title: "Empty",
            series_id: Some(complete),
            volume: Some(2),
            summary: None,
            isbn: Some(""),
            thumbnail_path: None,
            authors: &[],
            author: None,
            publish_date: None,
            language: None,
        },
    )
    .await;
    insert_book(
        &pool,
        library,
        BookSeed {
            title: "Partial",
            summary: None,
            isbn: None,
            ..BookSeed::default()
        },
    )
    .await;

    let summary = summary(&state, Some(library)).await;

    assert_eq!(summary.series_total, 2);
    assert_eq!(summary.series_no_description, 1);
    assert_eq!(summary.series_no_genre, 1);
    assert_eq!(summary.series_no_authors, 1);
    assert_eq!(summary.series_no_publishers, 1);
    assert_eq!(summary.series_no_year, 1);
    assert_eq!(summary.series_no_cover, 1);
    assert_eq!(summary.series_no_community_score, 2);
    assert_eq!(summary.books_total, 3);
    assert_eq!(summary.books_no_summary, 2);
    assert_eq!(summary.books_no_isbn, 2);
    assert_eq!(summary.books_no_cover, 1);
    assert_eq!(summary.books_no_author, 1);
    assert_eq!(summary.books_no_publish_date, 1);
    assert_eq!(summary.books_no_language, 1);
    assert_eq!(summary.books_no_volume, 1);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn gap_summary_scopes_counts_to_library(pool: PgPool) {
    let state = test_state(pool.clone());
    let lib_one = create_library(&pool, "one").await;
    let lib_two = create_library(&pool, "two").await;

    insert_series(
        &pool,
        lib_one,
        SeriesSeed {
            name: "A",
            ..Default::default()
        },
    )
    .await;
    insert_series(
        &pool,
        lib_two,
        SeriesSeed {
            name: "B",
            ..Default::default()
        },
    )
    .await;
    insert_series(
        &pool,
        lib_two,
        SeriesSeed {
            name: "C",
            ..Default::default()
        },
    )
    .await;

    assert_eq!(summary(&state, Some(lib_one)).await.series_total, 1);
    assert_eq!(summary(&state, Some(lib_two)).await.series_total, 2);
    assert_eq!(summary(&state, None).await.series_total, 3);
}

// ---------------------------------------------------------------------------
// series gap filters
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_gap_filters_isolate_each_dimension(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "series-gaps").await;

    insert_series(&pool, library, SeriesSeed::default()).await;
    let no_description = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "NoDescription",
            description: None,
            ..Default::default()
        },
    )
    .await;
    let no_genre = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "NoGenre",
            genres: &[],
            ..Default::default()
        },
    )
    .await;
    let no_authors = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "NoAuthors",
            authors: &[],
            ..Default::default()
        },
    )
    .await;
    let no_publishers = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "NoPublishers",
            publishers: &[],
            ..Default::default()
        },
    )
    .await;
    let no_year = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "NoYear",
            start_year: None,
            ..Default::default()
        },
    )
    .await;
    let no_cover = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "NoCover",
            cover_url: None,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(
        series_ids(&state, library, "no_description").await,
        vec![no_description]
    );
    assert_eq!(
        series_ids(&state, library, "no_genre").await,
        vec![no_genre]
    );
    assert_eq!(
        series_ids(&state, library, "no_authors").await,
        vec![no_authors]
    );
    assert_eq!(
        series_ids(&state, library, "no_publishers").await,
        vec![no_publishers]
    );
    assert_eq!(series_ids(&state, library, "no_year").await, vec![no_year]);
    assert_eq!(
        series_ids(&state, library, "no_cover").await,
        vec![no_cover]
    );
    assert_eq!(
        series_ids(&state, library, "not-a-gap").await.len(),
        7,
        "an unknown gap is ignored"
    );
    assert_eq!(
        series_ids(&state, library, "unlinked").await.len(),
        7,
        "a removed gap is ignored"
    );
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_no_community_score_gap_matches_summary(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "community-score-gaps").await;

    let rated = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "Rated",
            ..Default::default()
        },
    )
    .await;
    let unrated = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "Unrated",
            ..Default::default()
        },
    )
    .await;
    let zero = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "ZeroRating",
            ..Default::default()
        },
    )
    .await;
    let no_link = insert_series(
        &pool,
        library,
        SeriesSeed {
            name: "NoLink",
            ..Default::default()
        },
    )
    .await;

    // A real rating, an approved link without rating, a zero rating (ignored by
    // the `provider_rating > 0` predicate), and no approved link at all.
    insert_approved_link(&pool, library, rated, Some(8.0), Some(10.0)).await;
    insert_approved_link(&pool, library, unrated, None, None).await;
    insert_approved_link(&pool, library, zero, Some(0.0), Some(10.0)).await;

    assert_eq!(
        sorted(series_ids(&state, library, "no_community_score").await),
        sorted(vec![unrated, zero, no_link]),
    );
    assert_eq!(
        summary(&state, Some(library))
            .await
            .series_no_community_score,
        3
    );
}

// ---------------------------------------------------------------------------
// books gap filters
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "../../infra/migrations")]
async fn books_gap_filters_isolate_each_dimension(pool: PgPool) {
    let state = test_state(pool.clone());
    let library = create_library(&pool, "books-gaps").await;
    let series = insert_series(&pool, library, SeriesSeed::default()).await;

    insert_book(
        &pool,
        library,
        BookSeed {
            title: "Complete",
            series_id: Some(series),
            volume: Some(1),
            ..Default::default()
        },
    )
    .await;
    let no_summary = insert_book(
        &pool,
        library,
        BookSeed {
            title: "NoSummary",
            series_id: Some(series),
            volume: Some(2),
            summary: None,
            ..Default::default()
        },
    )
    .await;
    let no_isbn = insert_book(
        &pool,
        library,
        BookSeed {
            title: "NoIsbn",
            series_id: Some(series),
            volume: Some(3),
            isbn: Some(""),
            ..Default::default()
        },
    )
    .await;
    let no_cover = insert_book(
        &pool,
        library,
        BookSeed {
            title: "NoCover",
            series_id: Some(series),
            volume: Some(4),
            thumbnail_path: None,
            ..Default::default()
        },
    )
    .await;
    let no_author = insert_book(
        &pool,
        library,
        BookSeed {
            title: "NoAuthor",
            series_id: Some(series),
            volume: Some(5),
            authors: &[],
            author: None,
            ..Default::default()
        },
    )
    .await;
    let no_publish_date = insert_book(
        &pool,
        library,
        BookSeed {
            title: "NoDate",
            series_id: Some(series),
            volume: Some(6),
            publish_date: None,
            ..Default::default()
        },
    )
    .await;
    let no_language = insert_book(
        &pool,
        library,
        BookSeed {
            title: "NoLanguage",
            series_id: Some(series),
            volume: Some(7),
            language: None,
            ..Default::default()
        },
    )
    .await;
    let no_volume = insert_book(
        &pool,
        library,
        BookSeed {
            title: "NoVolume",
            series_id: Some(series),
            volume: None,
            ..Default::default()
        },
    )
    .await;

    assert_eq!(
        sorted(book_ids(&state, library, "no_summary").await),
        sorted(vec![no_summary])
    );
    assert_eq!(
        sorted(book_ids(&state, library, "no_isbn").await),
        sorted(vec![no_isbn])
    );
    assert_eq!(
        sorted(book_ids(&state, library, "no_cover").await),
        sorted(vec![no_cover])
    );
    assert_eq!(
        sorted(book_ids(&state, library, "no_author").await),
        sorted(vec![no_author])
    );
    assert_eq!(
        sorted(book_ids(&state, library, "no_publish_date").await),
        sorted(vec![no_publish_date])
    );
    assert_eq!(
        sorted(book_ids(&state, library, "no_language").await),
        sorted(vec![no_language])
    );
    assert_eq!(
        sorted(book_ids(&state, library, "no_volume").await),
        sorted(vec![no_volume])
    );
    assert_eq!(book_ids(&state, library, "not-a-gap").await.len(), 8);
    assert_eq!(book_ids(&state, library, "missing_volumes").await.len(), 8);
}
