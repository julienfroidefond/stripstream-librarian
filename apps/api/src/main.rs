mod auth;
mod authors;
mod books;
mod error;
mod handlers;
mod index_jobs;
mod job_poller;
mod komga;
mod libraries;
mod metadata;
mod metadata_batch;
mod metadata_refresh;
mod metadata_providers;
mod api_middleware;
mod openapi;
mod pages;
mod prowlarr;
mod qbittorrent;
mod reading_progress;
mod search;
mod series;
mod settings;
mod state;
mod stats;
mod telegram;
mod thumbnails;
mod tokens;

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
    };

    let admin_routes = Router::new()
        .route("/libraries", axum::routing::post(libraries::create_library))
        .route("/libraries/:id", delete(libraries::delete_library))
        .route("/libraries/:id/monitoring", axum::routing::patch(libraries::update_monitoring))
        .route("/libraries/:id/metadata-provider", axum::routing::patch(libraries::update_metadata_provider))
        .route("/books/:id", axum::routing::patch(books::update_book))
        .route("/books/:id/convert", axum::routing::post(books::convert_book))
        .route("/libraries/:library_id/series/:name", axum::routing::patch(series::update_series))
        .route("/index/rebuild", axum::routing::post(index_jobs::enqueue_rebuild))
        .route("/index/thumbnails/rebuild", axum::routing::post(thumbnails::start_thumbnails_rebuild))
        .route("/index/thumbnails/regenerate", axum::routing::post(thumbnails::start_thumbnails_regenerate))
        .route("/index/status", get(index_jobs::list_index_jobs))
        .route("/index/jobs/active", get(index_jobs::get_active_jobs))
        .route("/index/jobs/:id", get(index_jobs::get_job_details))
        .route("/index/jobs/:id/stream", get(index_jobs::stream_job_progress))
        .route("/index/jobs/:id/errors", get(index_jobs::get_job_errors))
        .route("/index/cancel/:id", axum::routing::post(index_jobs::cancel_job))
        .route("/folders", get(index_jobs::list_folders))
        .route("/admin/tokens", get(tokens::list_tokens).post(tokens::create_token))
        .route("/admin/tokens/:id", delete(tokens::revoke_token))
        .route("/admin/tokens/:id/delete", axum::routing::post(tokens::delete_token))
        .route("/prowlarr/search", axum::routing::post(prowlarr::search_prowlarr))
        .route("/prowlarr/test", get(prowlarr::test_prowlarr))
        .route("/qbittorrent/add", axum::routing::post(qbittorrent::add_torrent))
        .route("/qbittorrent/test", get(qbittorrent::test_qbittorrent))
        .route("/telegram/test", get(telegram::test_telegram))
        .route("/komga/sync", axum::routing::post(komga::sync_komga_read_books))
        .route("/komga/reports", get(komga::list_sync_reports))
        .route("/komga/reports/:id", get(komga::get_sync_report))
        .route("/metadata/search", axum::routing::post(metadata::search_metadata))
        .route("/metadata/match", axum::routing::post(metadata::create_metadata_match))
        .route("/metadata/approve/:id", axum::routing::post(metadata::approve_metadata))
        .route("/metadata/reject/:id", axum::routing::post(metadata::reject_metadata))
        .route("/metadata/links", get(metadata::get_metadata_links))
        .route("/metadata/missing/:id", get(metadata::get_missing_books))
        .route("/metadata/links/:id", delete(metadata::delete_metadata_link))
        .route("/metadata/batch", axum::routing::post(metadata_batch::start_batch))
        .route("/metadata/batch/:id/report", get(metadata_batch::get_batch_report))
        .route("/metadata/batch/:id/results", get(metadata_batch::get_batch_results))
        .route("/metadata/refresh", axum::routing::post(metadata_refresh::start_refresh))
        .route("/metadata/refresh/:id/report", get(metadata_refresh::get_refresh_report))
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
        .route("/books/:id/pages/:n", get(pages::get_page))
        .route("/books/:id/progress", get(reading_progress::get_reading_progress).patch(reading_progress::update_reading_progress))
        .route("/libraries/:library_id/series", get(series::list_series))
        .route("/libraries/:library_id/series/:name/metadata", get(series::get_series_metadata))
        .route("/series", get(series::list_all_series))
        .route("/series/ongoing", get(series::ongoing_series))
        .route("/series/statuses", get(series::series_statuses))
        .route("/series/provider-statuses", get(series::provider_statuses))
        .route("/series/mark-read", axum::routing::post(reading_progress::mark_series_read))
        .route("/authors", get(authors::list_authors))
        .route("/stats", get(stats::get_stats))
        .route("/search", get(search::search_books))
        .route_layer(middleware::from_fn_with_state(state.clone(), api_middleware::read_rate_limit))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_read,
        ));

    // Clone pool before state is moved into the router
    let poller_pool = state.pool.clone();

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .route("/metrics", get(handlers::metrics))
        .route("/docs", get(handlers::docs_redirect))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", openapi::ApiDoc::openapi()))
        .merge(admin_routes)
        .merge(read_routes)
        .layer(middleware::from_fn_with_state(state.clone(), api_middleware::request_counter))
        .with_state(state);

    // Start background poller for API-only jobs (metadata_batch, metadata_refresh)
    tokio::spawn(async move {
        job_poller::run_job_poller(poller_pool, 5).await;
    });

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

