use axum::{routing::get, Router};
use stripstream_core::config::IndexerConfig;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "indexer=info,axum=info".to_string()),
        )
        .init();

    let config = IndexerConfig::from_env();
    let app = Router::new().route("/health", get(health));

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "indexer listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}
