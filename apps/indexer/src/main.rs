use axum::{routing::get, Router};
use indexer::{api, AppState};
use sqlx::postgres::PgPoolOptions;
use stripstream_core::config::IndexerConfig;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "indexer=info,axum=info".to_string()),
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
        api_base_url: config.api_base_url.clone(),
        api_bootstrap_token: config.api_bootstrap_token.clone(),
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
