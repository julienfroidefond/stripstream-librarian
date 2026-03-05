use axum::{response::Html, routing::get, Router};
use stripstream_core::config::AdminUiConfig;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "admin_ui=info,axum=info".to_string()),
        )
        .init();

    let config = AdminUiConfig::from_env();
    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(index));

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "admin ui listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn index() -> Html<&'static str> {
    Html(
        "<html><body><h1>Stripstream Admin</h1><p>UI skeleton ready. Next: libraries, jobs, tokens screens.</p></body></html>",
    )
}
