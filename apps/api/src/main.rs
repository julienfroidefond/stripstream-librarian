mod auth;
mod books;
mod error;
mod handlers;
mod index_jobs;
mod komga;
mod libraries;
mod api_middleware;
mod openapi;
mod pages;
mod reading_progress;
mod search;
mod settings;
mod state;
mod stats;
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
        .route("/libraries", get(libraries::list_libraries).post(libraries::create_library))
        .route("/libraries/:id", delete(libraries::delete_library))
        .route("/libraries/:id/scan", axum::routing::post(libraries::scan_library))
        .route("/libraries/:id/monitoring", axum::routing::patch(libraries::update_monitoring))
        .route("/books/:id", axum::routing::patch(books::update_book))
        .route("/books/:id/convert", axum::routing::post(books::convert_book))
        .route("/libraries/:library_id/series/:name", axum::routing::patch(books::update_series))
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
        .route("/komga/sync", axum::routing::post(komga::sync_komga_read_books))
        .route("/komga/reports", get(komga::list_sync_reports))
        .route("/komga/reports/:id", get(komga::get_sync_report))
        .merge(settings::settings_routes())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));

    let read_routes = Router::new()
        .route("/books", get(books::list_books))
        .route("/books/ongoing", get(books::ongoing_books))
        .route("/books/:id", get(books::get_book))
        .route("/books/:id/thumbnail", get(books::get_thumbnail))
        .route("/books/:id/pages/:n", get(pages::get_page))
        .route("/books/:id/progress", get(reading_progress::get_reading_progress).patch(reading_progress::update_reading_progress))
        .route("/libraries/:library_id/series", get(books::list_series))
        .route("/libraries/:library_id/series/:name/metadata", get(books::get_series_metadata))
        .route("/series", get(books::list_all_series))
        .route("/series/ongoing", get(books::ongoing_series))
        .route("/series/mark-read", axum::routing::post(reading_progress::mark_series_read))
        .route("/stats", get(stats::get_stats))
        .route("/search", get(search::search_books))
        .route_layer(middleware::from_fn_with_state(state.clone(), api_middleware::read_rate_limit))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_read,
        ));

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

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

