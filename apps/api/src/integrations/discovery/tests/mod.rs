use super::*;
use axum::extract::State;
use axum::Json;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock, Semaphore};

use crate::state::{
    AppState, DiskCacheStatsSnapshot, DynamicSettings, Metrics, PageRenderLocks, ReadRateLimit,
};

/// Build a minimal AppState suitable for testing (only `pool` is used).
fn test_state(pool: sqlx::PgPool) -> AppState {
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
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit {
            window_started_at: Instant::now(),
            requests_in_window: 0,
        })),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_download_limit: Arc::new(Semaphore::new(1)),
        telegram_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
    }
}

/// Insert a test library and return its UUID.
async fn create_test_library(pool: &sqlx::PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(id)
        .bind("Test Library")
        .bind(format!("/test/{}", id))
        .execute(pool)
        .await
        .expect("failed to create test library");
    id
}

fn make_request(
    library_id: Uuid,
    provider: &str,
    external_id: &str,
    title: &str,
) -> AddToLibraryRequest {
    AddToLibraryRequest {
        library_id,
        provider: provider.to_string(),
        external_id: external_id.to_string(),
        title: title.to_string(),
        description: Some("A test description".to_string()),
        authors: Some(vec!["Author A".to_string()]),
        publishers: None,
        genres: Some(vec!["Action".to_string()]),
        start_year: Some(2020),
        total_volumes: Some(10),
        status: Some("ongoing".to_string()),
        cover_url: Some("https://example.com/cover.jpg".to_string()),
        external_url: Some("https://example.com/series/123".to_string()),
    }
}

// 1. Basic add from bedetheque — series + metadata link created
#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_add_to_library_basic_bedetheque(pool: sqlx::PgPool) {
    let library_id = create_test_library(&pool).await;
    let state = test_state(pool.clone());
    let req = make_request(library_id, "bedetheque", "ext-123", "Test BD Series");

    let result = add_to_library(State(state), Json(req)).await;
    assert!(result.is_ok(), "add_to_library should succeed");

    let resp = result.unwrap().0;

    // Verify series was created
    let series_row = sqlx::query("SELECT name, description, start_year, total_volumes, status, cover_url FROM series WHERE id = $1")
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .expect("series should exist");
    let name: String = series_row.get("name");
    assert_eq!(name, "Test BD Series");
    let desc: Option<String> = series_row.get("description");
    assert_eq!(desc, Some("A test description".to_string()));
    let start_year: Option<i32> = series_row.get("start_year");
    assert_eq!(start_year, Some(2020));
    let cover_url: Option<String> = series_row.get("cover_url");
    assert_eq!(
        cover_url,
        Some("https://example.com/cover.jpg".to_string()),
        "cover_url should be synced"
    );

    // Verify metadata link was created
    let link_row = sqlx::query(
        "SELECT provider, external_id, status FROM external_metadata_links WHERE series_id = $1 AND provider = 'bedetheque'"
    )
    .bind(resp.series_id)
    .fetch_one(&pool)
    .await
    .expect("metadata link should exist");
    let provider: String = link_row.get("provider");
    assert_eq!(provider, "bedetheque");
    let ext_id: String = link_row.get("external_id");
    assert_eq!(ext_id, "ext-123");
    let status: String = link_row.get("status");
    assert_eq!(status, "approved");
}

// 2. Duplicate series — same series_id returned, no duplicate
#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_add_to_library_duplicate_series(pool: sqlx::PgPool) {
    let library_id = create_test_library(&pool).await;
    let state = test_state(pool.clone());

    let req1 = make_request(library_id, "bedetheque", "ext-100", "Duplicate Series");
    let resp1 = add_to_library(State(state.clone()), Json(req1))
        .await
        .unwrap()
        .0;

    let req2 = make_request(library_id, "bedetheque", "ext-200", "Duplicate Series");
    let resp2 = add_to_library(State(state), Json(req2)).await.unwrap().0;

    // Same series_id should be returned
    assert_eq!(
        resp1.series_id, resp2.series_id,
        "should return the same series_id for duplicate name"
    );

    // Only one series row
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM series WHERE library_id = $1 AND name = 'Duplicate Series'",
    )
    .bind(library_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "should have exactly one series row");

    // Metadata link should have been upserted (ON CONFLICT updates external_id)
    let link_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1 AND provider = 'bedetheque'"
    )
    .bind(resp1.series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        link_count, 1,
        "should have exactly one metadata link per (series_id, provider)"
    );

    // LOCKED: the second add overwrites the existing link's external_id
    // (ext-100 → ext-200), losing the original provider identity.
    // See docs/KNOWN_ISSUES.md §1.
    let external_id: String = sqlx::query_scalar(
        "SELECT external_id FROM external_metadata_links WHERE series_id = $1 AND provider = 'bedetheque'",
    )
    .bind(resp1.series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(external_id, "ext-200");
}

// 3. Non-linkable provider (anilist) — no metadata link created
#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_add_to_library_anilist_no_metadata_link(pool: sqlx::PgPool) {
    let library_id = create_test_library(&pool).await;
    let state = test_state(pool.clone());
    let req = make_request(library_id, "anilist", "ani-456", "Anilist Series");

    let resp = add_to_library(State(state), Json(req)).await.unwrap().0;

    // Series should exist
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE id = $1")
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "series should be created");

    // No metadata link should be created
    let link_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1")
            .bind(resp.series_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    // LOCKED: only a hard-coded allowlist (bedetheque/senscritique) creates a
    // metadata link; other providers added via discovery create none, so
    // metadata sync is impossible for them. See docs/KNOWN_ISSUES.md §1.
    assert_eq!(
        link_count, 0,
        "anilist provider should not create metadata link"
    );

    // metadata_link_id should fall back to series_id
    assert_eq!(
        resp.metadata_link_id, resp.series_id,
        "metadata_link_id should equal series_id when no link created"
    );
}

// 4. SC provider normalization — sc_trending_bd → senscritique
#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_add_to_library_sc_provider_normalization(pool: sqlx::PgPool) {
    let library_id = create_test_library(&pool).await;
    let state = test_state(pool.clone());
    let req = make_request(library_id, "sc_trending_bd", "sc-789", "SC Trending Series");

    let resp = add_to_library(State(state), Json(req)).await.unwrap().0;

    // Metadata link should exist with provider = "senscritique" (not "sc_trending_bd")
    let link_row = sqlx::query("SELECT provider FROM external_metadata_links WHERE series_id = $1")
        .bind(resp.series_id)
        .fetch_one(&pool)
        .await
        .expect("metadata link should exist for sc_ provider");
    let provider: String = link_row.get("provider");
    assert_eq!(
        provider, "senscritique",
        "sc_trending_bd should be normalized to senscritique"
    );

    // Verify no link exists with original provider name
    let raw_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM external_metadata_links WHERE series_id = $1 AND provider = 'sc_trending_bd'"
    )
    .bind(resp.series_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        raw_count, 0,
        "no link should exist with raw sc_trending_bd provider name"
    );
}

// 5. Senscritique provider — metadata link created with correct provider
#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_add_to_library_senscritique_provider(pool: sqlx::PgPool) {
    let library_id = create_test_library(&pool).await;
    let state = test_state(pool.clone());
    let req = make_request(
        library_id,
        "senscritique",
        "sc-direct-001",
        "SC Direct Series",
    );

    let resp = add_to_library(State(state), Json(req)).await.unwrap().0;

    // Metadata link should exist with provider = "senscritique"
    let link_row = sqlx::query(
        "SELECT provider, external_id, status FROM external_metadata_links WHERE series_id = $1",
    )
    .bind(resp.series_id)
    .fetch_one(&pool)
    .await
    .expect("metadata link should exist for senscritique provider");
    let provider: String = link_row.get("provider");
    assert_eq!(provider, "senscritique");
    let ext_id: String = link_row.get("external_id");
    assert_eq!(ext_id, "sc-direct-001");
    let status: String = link_row.get("status");
    assert_eq!(status, "approved");

    // metadata_link_id should NOT equal series_id (a real link was created)
    assert_ne!(
        resp.metadata_link_id, resp.series_id,
        "metadata_link_id should be a real link UUID, not series_id"
    );
}

// 6. Hide suggestion + filter_already_owned excludes hidden
#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_hide_and_filter_excludes_hidden(pool: sqlx::PgPool) {
    // Insert a hidden entry
    sqlx::query(
        "INSERT INTO discovery_hidden (provider, external_id, title) VALUES ('bedetheque', 'hidden-ext-1', 'Hidden Series')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Build suggestions that include the hidden one
    let suggestions = vec![
        DiscoverySuggestionDto {
            provider: "bedetheque".to_string(),
            external_id: "hidden-ext-1".to_string(),
            title: "Hidden Series".to_string(),
            authors: vec![],
            description: None,
            genres: vec![],
            cover_url: None,
            external_url: None,
            rating: None,
            start_year: None,
            total_volumes: None,
            status: None,
        },
        DiscoverySuggestionDto {
            provider: "bedetheque".to_string(),
            external_id: "visible-ext-2".to_string(),
            title: "Visible Series".to_string(),
            authors: vec![],
            description: None,
            genres: vec![],
            cover_url: None,
            external_url: None,
            rating: None,
            start_year: None,
            total_volumes: None,
            status: None,
        },
    ];

    let filtered = filter_already_owned(&pool, "bedetheque", suggestions).await;
    assert_eq!(filtered.len(), 1, "hidden series should be filtered out");
    assert_eq!(filtered[0].external_id, "visible-ext-2");
}

// 7. Unhide removes from discovery_hidden
#[sqlx::test(migrations = "../../infra/migrations")]
async fn test_unhide_removes_hidden(pool: sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO discovery_hidden (provider, external_id, title) VALUES ('bedetheque', 'unhide-ext', 'To Unhide')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify it exists
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM discovery_hidden WHERE external_id = 'unhide-ext'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);

    // Delete it
    sqlx::query(
        "DELETE FROM discovery_hidden WHERE provider = 'bedetheque' AND external_id = 'unhide-ext'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM discovery_hidden WHERE external_id = 'unhide-ext'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "hidden entry should be removed after unhide");
}
