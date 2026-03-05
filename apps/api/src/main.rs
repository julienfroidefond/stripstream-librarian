mod auth;
mod books;
mod error;
mod index_jobs;
mod libraries;
mod openapi;
mod pages;
mod search;
mod tokens;

use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use axum::{
    middleware,
    response::IntoResponse,
    routing::{delete, get},
    Json, Router,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use lru::LruCache;
use stripstream_core::config::ApiConfig;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::{Mutex, Semaphore};
use tracing::info;

#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
    bootstrap_token: Arc<str>,
    meili_url: Arc<str>,
    meili_master_key: Arc<str>,
    page_cache: Arc<Mutex<LruCache<String, Arc<Vec<u8>>>>>,
    page_render_limit: Arc<Semaphore>,
    metrics: Arc<Metrics>,
    read_rate_limit: Arc<Mutex<ReadRateLimit>>,
}

struct Metrics {
    requests_total: AtomicU64,
    page_cache_hits: AtomicU64,
    page_cache_misses: AtomicU64,
}

struct ReadRateLimit {
    window_started_at: Instant,
    requests_in_window: u32,
}

impl Metrics {
    fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            page_cache_hits: AtomicU64::new(0),
            page_cache_misses: AtomicU64::new(0),
        }
    }
}

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

    let state = AppState {
        pool,
        bootstrap_token: Arc::from(config.api_bootstrap_token),
        meili_url: Arc::from(config.meili_url),
        meili_master_key: Arc::from(config.meili_master_key),
        page_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(512).expect("non-zero")))),
        page_render_limit: Arc::new(Semaphore::new(4)),
        metrics: Arc::new(Metrics::new()),
        read_rate_limit: Arc::new(Mutex::new(ReadRateLimit {
            window_started_at: Instant::now(),
            requests_in_window: 0,
        })),
    };

    let admin_routes = Router::new()
        .route("/libraries", get(libraries::list_libraries).post(libraries::create_library))
        .route("/libraries/:id", delete(libraries::delete_library))
        .route("/index/rebuild", axum::routing::post(index_jobs::enqueue_rebuild))
        .route("/index/status", get(index_jobs::list_index_jobs))
        .route("/index/cancel/:id", axum::routing::post(index_jobs::cancel_job))
        .route("/folders", get(index_jobs::list_folders))
        .route("/admin/tokens", get(tokens::list_tokens).post(tokens::create_token))
        .route("/admin/tokens/:id", delete(tokens::revoke_token))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));

    let read_routes = Router::new()
        .route("/books", get(books::list_books))
        .route("/books/:id", get(books::get_book))
        .route("/books/:id/pages/:n", get(pages::get_page))
        .route("/search", get(search::search_books))
        .route_layer(middleware::from_fn_with_state(state.clone(), read_rate_limit))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_read,
        ));

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/docs", get(docs_redirect))
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", openapi::ApiDoc::openapi()))
        .merge(admin_routes)
        .merge(read_routes)
        .layer(middleware::from_fn_with_state(state.clone(), request_counter))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn docs_redirect() -> impl axum::response::IntoResponse {
    axum::response::Redirect::to("/swagger-ui/")
}

async fn ready(axum::extract::State(state): axum::extract::State<AppState>) -> Result<Json<serde_json::Value>, error::ApiError> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(Json(serde_json::json!({"status": "ready"})))
}

async fn metrics(axum::extract::State(state): axum::extract::State<AppState>) -> String {
    format!(
        "requests_total {}\npage_cache_hits {}\npage_cache_misses {}\n",
        state.metrics.requests_total.load(Ordering::Relaxed),
        state.metrics.page_cache_hits.load(Ordering::Relaxed),
        state.metrics.page_cache_misses.load(Ordering::Relaxed),
    )
}

async fn request_counter(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    next.run(req).await
}

async fn read_rate_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut limiter = state.read_rate_limit.lock().await;
    if limiter.window_started_at.elapsed() >= Duration::from_secs(1) {
        limiter.window_started_at = Instant::now();
        limiter.requests_in_window = 0;
    }

    if limiter.requests_in_window >= 120 {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded",
        )
            .into_response();
    }

    limiter.requests_in_window += 1;
    drop(limiter);
    next.run(req).await
}
