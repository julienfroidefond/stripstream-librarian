mod authors;
mod books;
mod downloads;
mod error;
mod genres;
mod handlers;
mod integrations;
mod jobs;
mod libraries;
mod metadata;
mod metadata_providers;
mod api_middleware;
mod openapi;
mod reading;
mod reading_lists;
mod responses;
mod search;
mod series;
mod settings;
mod state;
mod stats;
mod users;

// Backward-compatible re-exports so existing `crate::auth::`, `crate::index_jobs::`,
// `crate::job_helpers::`, `crate::tokens::` paths keep working.
pub(crate) use users::auth;
pub(crate) use users::tokens;
pub(crate) use jobs::index_jobs;
pub(crate) use jobs::helpers as job_helpers;
pub(crate) use jobs::poller as job_poller;

use std::sync::Arc;
use std::time::Instant;

use axum::{
    middleware,
    routing::{delete, get},
    Router,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use lru::LruCache;
use std::num::NonZeroUsize;
use stripstream_core::config::ApiConfig;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::info;

use crate::state::{load_concurrent_renders, load_dynamic_settings, AppState, Metrics, ReadRateLimit};

#[tokio::main]
#[allow(deprecated)] // Old library-scoped series routes kept for backward compat
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "api=info,axum=info".to_string()),
        )
        .init();

    let config = ApiConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    // Load concurrent_renders from settings, default to 8
    let concurrent_renders = load_concurrent_renders(&pool).await;
    info!("Using concurrent_renders limit: {}", concurrent_renders);

    let dynamic_settings = load_dynamic_settings(&pool).await;
    info!(
        "Dynamic settings: rate_limit={}, timeout={}s, format={}, quality={}, filter={}, max_width={}, cache_dir={}",
        dynamic_settings.rate_limit_per_second,
        dynamic_settings.timeout_seconds,
        dynamic_settings.image_format,
        dynamic_settings.image_quality,
        dynamic_settings.image_filter,
        dynamic_settings.image_max_width,
        dynamic_settings.cache_directory,
    );

    let state = AppState {
        pool,
        bootstrap_token: Arc::from(config.api_bootstrap_token),
        page_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(512).expect("non-zero")))),
        page_render_limit: Arc::new(Semaphore::new(concurrent_renders)),
        metrics: Arc::new(Metrics::new()),
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit {
            window_started_at: Instant::now(),
            requests_in_window: 0,
        })),
        settings: Arc::new(RwLock::new(dynamic_settings)),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
    };

    let admin_routes = Router::new()
        .route("/libraries", axum::routing::post(libraries::create_library))
        .route("/libraries/:id", delete(libraries::delete_library))
        .route("/libraries/:id/monitoring", axum::routing::patch(libraries::update_monitoring))
        .route("/libraries/:id/metadata-provider", axum::routing::patch(libraries::update_metadata_provider))
        .route("/libraries/:id/reading-status-provider", axum::routing::patch(libraries::update_reading_status_provider))
        .route("/libraries/:id/tags", axum::routing::patch(libraries::update_tags))
        .route("/books/:id", axum::routing::patch(books::update_book).delete(books::delete_book))
        .route("/books/:id/convert", axum::routing::post(books::convert_book))
        .route("/libraries/:library_id/series/:series_id", axum::routing::patch(series::update_series).delete(series::delete_series))
        .route("/series/:series_id", axum::routing::patch(series::update_series_by_id).delete(series::delete_series_by_id))
        .route("/series/:series_id/rename-books", axum::routing::post(books::rename_books))
        .route("/series/create", axum::routing::post(series::create_series))
        .route("/series/:series_id/merge", axum::routing::post(series::merge_series))
        .route("/index/rebuild", axum::routing::post(index_jobs::enqueue_rebuild))
        .route("/index/thumbnails/rebuild", axum::routing::post(books::start_thumbnails_rebuild))
        .route("/index/thumbnails/regenerate", axum::routing::post(books::start_thumbnails_regenerate))
        .route("/index/status", get(index_jobs::list_index_jobs))
        .route("/index/jobs/active", get(index_jobs::get_active_jobs))
        .route("/index/jobs/:id", get(index_jobs::get_job_details))
        .route("/index/jobs/:id/stream", get(index_jobs::stream_job_progress))
        .route("/index/jobs/:id/errors", get(index_jobs::get_job_errors))
        .route("/index/jobs/:id/indexed-books", get(index_jobs::get_indexed_books))
        .route("/index/jobs/:id/events", get(index_jobs::get_job_events))
        .route("/index/cancel/:id", axum::routing::post(index_jobs::cancel_job))
        .route("/folders", get(index_jobs::list_folders))
        .route("/admin/users", get(users::accounts::list_users).post(users::accounts::create_user))
        .route("/admin/users/:id", delete(users::accounts::delete_user).patch(users::accounts::update_user))
        .route("/admin/tokens", get(tokens::list_tokens).post(tokens::create_token))
        .route("/admin/tokens/:id", delete(tokens::revoke_token).patch(tokens::update_token))
        .route("/admin/tokens/:id/delete", axum::routing::post(tokens::delete_token))
        .route("/prowlarr/search", axum::routing::post(downloads::search_prowlarr))
        .route("/prowlarr/test", get(downloads::test_prowlarr))
        .route("/qbittorrent/add", axum::routing::post(downloads::add_torrent))
        .route("/qbittorrent/test", get(downloads::test_qbittorrent))
        .route("/torrent-downloads", get(downloads::list_torrent_downloads))
        .route("/torrent-downloads/:id", axum::routing::delete(downloads::delete_torrent_download))
        .route("/torrent-downloads/:id/retry", axum::routing::post(downloads::retry_torrent_import))
        .route("/telegram/test", get(integrations::telegram::test_telegram))
        .route("/komga/sync", axum::routing::post(integrations::komga::sync_komga_read_books))
        .route("/komga/reports", get(integrations::komga::list_sync_reports))
        .route("/komga/reports/:id", get(integrations::komga::get_sync_report))
        .route("/anilist/status", get(integrations::anilist::get_status))
        .route("/anilist/search", axum::routing::post(integrations::anilist::search_manga))
        .route("/anilist/unlinked", get(integrations::anilist::list_unlinked))
        .route("/anilist/sync/preview", get(integrations::anilist::preview_sync))
        .route("/anilist/sync", axum::routing::post(integrations::anilist::sync_to_anilist))
        .route("/anilist/pull", axum::routing::post(integrations::anilist::pull_from_anilist))
        .route("/anilist/links", get(integrations::anilist::list_links))
        .route("/anilist/libraries/:id", axum::routing::patch(integrations::anilist::toggle_library))
        .route("/anilist/series/:library_id/:series_name", get(integrations::anilist::get_series_link))
        .route("/anilist/series/:library_id/:series_name/link", axum::routing::post(integrations::anilist::link_series))
        .route("/anilist/series/:library_id/:series_name/unlink", delete(integrations::anilist::unlink_series))
        .route("/series/:series_id/anilist", get(integrations::anilist::get_series_link_by_id))
        .route("/series/:series_id/anilist/link", axum::routing::post(integrations::anilist::link_series_by_id))
        .route("/series/:series_id/anilist/unlink", delete(integrations::anilist::unlink_series_by_id))
        .route("/metadata/search", axum::routing::post(metadata::search_metadata))
        .route("/metadata/match", axum::routing::post(metadata::create_metadata_match))
        .route("/metadata/approve/:id", axum::routing::post(metadata::approve_metadata))
        .route("/metadata/reject/:id", axum::routing::post(metadata::reject_metadata))
        .route("/metadata/links/:id", delete(metadata::delete_metadata_link))
        .route("/metadata/batch", axum::routing::post(metadata::start_batch))
        .route("/metadata/batch/:id/report", get(metadata::get_batch_report))
        .route("/metadata/batch/:id/results", get(metadata::get_batch_results))
        .route("/metadata/refresh", axum::routing::post(metadata::start_refresh))
        .route("/metadata/refresh-all", axum::routing::post(metadata::start_refresh_all))
        .route("/metadata/refresh-link/:id", axum::routing::post(metadata::refresh_single_link))
        .route("/metadata/refresh/:id/report", get(metadata::get_refresh_report))
        .route("/reading-status/match", axum::routing::post(reading::start_match))
        .route("/reading-status/match/:id/report", get(reading::get_match_report))
        .route("/reading-status/match/:id/results", get(reading::get_match_results))
        .route("/reading-status/push", axum::routing::post(reading::start_push))
        .route("/reading-status/push/:id/report", get(reading::get_push_report))
        .route("/reading-status/push/:id/results", get(reading::get_push_results))
        .route("/download-detection/start", axum::routing::post(downloads::start_detection))
        .route("/download-detection/latest-found", get(downloads::get_latest_found))
        .route("/download-detection/:id/report", get(downloads::get_detection_report))
        .route("/download-detection/:id/results", get(downloads::get_detection_results))
        .route("/available-downloads/:id", axum::routing::delete(downloads::delete_available_download))
        .route("/release-blacklist", get(downloads::list_blacklisted_releases))
        .route("/release-blacklist", axum::routing::post(downloads::blacklist_release))
        .route("/release-blacklist/:id", axum::routing::delete(downloads::unblacklist_release))
        .route("/discovery/trending", get(integrations::discovery::trending))
        .route("/discovery/prowlarr", get(integrations::discovery::prowlarr_discovery))
        .route("/discovery/add-to-library", axum::routing::post(integrations::discovery::add_to_library))
        .route("/discovery/hide", axum::routing::post(integrations::discovery::hide_suggestion))
        .route("/discovery/unhide", axum::routing::post(integrations::discovery::unhide_suggestion))
        .route("/discovery/hidden", get(integrations::discovery::list_hidden))
        .route("/genres", get(genres::list_genres))
        .route("/genres/assign", axum::routing::post(genres::assign_genre))
        .route("/genres/untagged-series", get(genres::untagged_series))
        .route("/genres/:name", axum::routing::patch(genres::rename_genre).delete(genres::delete_genre))
        .route("/reading-lists", axum::routing::post(reading_lists::create_reading_list))
        .route("/reading-lists/:id", axum::routing::patch(reading_lists::update_reading_list).delete(reading_lists::delete_reading_list))
        .route("/reading-lists/:id/series", axum::routing::post(reading_lists::add_series))
        .route("/reading-lists/:id/series/:series_id", delete(reading_lists::remove_series))
        .route("/reading-lists/:id/series/reorder", axum::routing::put(reading_lists::reorder_series))
        .merge(settings::settings_routes())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));

    let read_routes = Router::new()
        .route("/libraries", get(libraries::list_libraries))
        .route("/libraries/:id/scan", axum::routing::post(libraries::scan_library))
        .route("/books", get(books::list_books))
        .route("/books/ongoing", get(series::ongoing_books))
        .route("/books/:id", get(books::get_book))
        .route("/books/:id/thumbnail", get(books::get_thumbnail))
        .route("/books/:id/pages/:n", get(books::get_page))
        .route("/books/:id/progress", get(reading::get_reading_progress).patch(reading::update_reading_progress))
        .route("/libraries/:library_id/series", get(series::list_series))
        .route("/libraries/:library_id/series/by-name/:name", get(series::get_series_by_name))
        .route("/libraries/:library_id/series/:series_id/metadata", get(series::get_series_metadata))
        .route("/series", get(series::list_all_series))
        .route("/series/:series_id/details", get(series::get_series_by_id))
        .route("/series/:series_id/metadata", get(series::get_series_metadata_by_id))
        .route("/series/:series_id/related", get(series::get_related_series))
        .route("/series/ongoing", get(series::ongoing_series))
        .route("/series/statuses", get(series::series_statuses))
        .route("/series/genres", get(series::series_genres))
        .route("/series/recommendations", get(series::get_recommendations))
        .route("/series/provider-statuses", get(series::provider_statuses))
        .route("/series/mark-read", axum::routing::post(reading::mark_series_read))
        .route("/authors", get(authors::list_authors))
        .route("/stats", get(stats::get_stats))
        .route("/search", get(search::search_books))
        .route("/metadata/links", get(metadata::get_metadata_links))
        .route("/metadata/missing/:id", get(metadata::get_missing_books))
        .route("/reading-lists", get(reading_lists::list_reading_lists))
        .route("/reading-lists/:id", get(reading_lists::get_reading_list))
        .route_layer(middleware::from_fn_with_state(state.clone(), api_middleware::read_rate_limit))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_read,
        ));

    // Clone pool before state is moved into the router
    let poller_pool = state.pool.clone();
    let torrent_poller_pool = state.pool.clone();

    let app = Router::new()
        .route("/", get(handlers::api_home))
        .route("/health", get(handlers::health))
        .route("/version", get(handlers::version))
        .route("/ready", get(handlers::ready))
        .route("/metrics", get(handlers::metrics))
        .route("/docs", get(handlers::docs_redirect))
        .route("/torrent-downloads/notify", axum::routing::post(downloads::notify_torrent_done))
        .merge(SwaggerUi::new("/swagger-ui")
            .url("/openapi.json", openapi::ClientApiDoc::openapi())
            .url("/admin/openapi.json", openapi::AdminApiDoc::openapi())
        )
        .merge(admin_routes)
        .merge(read_routes)
        .layer(middleware::from_fn_with_state(state.clone(), api_middleware::request_counter))
        .with_state(state);

    // Start background poller for API-only jobs (metadata_batch, metadata_refresh)
    tokio::spawn(async move {
        job_poller::run_job_poller(poller_pool, 5).await;
    });

    // Start background poller for qBittorrent torrent completions (every 30s)
    tokio::spawn(async move {
        downloads::run_torrent_poller(torrent_poller_pool, 30).await;
    });

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

