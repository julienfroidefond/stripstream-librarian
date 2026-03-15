use axum::{routing::get, Router};
use indexer::{api, AppState};
use sqlx::postgres::PgPoolOptions;
use stripstream_core::config::IndexerConfig;
use tracing::info;

fn main() -> anyhow::Result<()> {
    // Limit blocking thread pool to 8 threads (default 512).
    // Each spawn_blocking call (archive extraction, image save) gets a thread.
    // With thousands of books, unlimited threads cause OOM via stack memory (~8MB each).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(8)
        .build()?;
    runtime.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| {
                "indexer=info,axum=info,scan=info,extraction=info,thumbnail=warn,watcher=info".to_string()
            }),
        )
        .init();

    let config = IndexerConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await?;

    let state = AppState {
        pool,
        meili_url: config.meili_url.clone(),
        meili_master_key: config.meili_master_key.clone(),
    };

    tokio::spawn(indexer::worker::run_worker(state.clone(), config.scan_interval_seconds));

    let app = Router::new()
        .route("/health", get(api::health))
        .route("/ready", get(api::ready))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "indexer listening");
    axum::serve(listener, app).await?;
    Ok(())
}
