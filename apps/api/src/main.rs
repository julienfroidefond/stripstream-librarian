mod auth;
mod error;
mod libraries;
mod tokens;

use std::sync::Arc;

use axum::{middleware, routing::{delete, get}, Router};
use stripstream_core::config::ApiConfig;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

#[derive(Clone)]
struct AppState {
    pool: sqlx::PgPool,
    bootstrap_token: Arc<str>,
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
    };

    let protected = Router::new()
        .route("/libraries", get(libraries::list_libraries).post(libraries::create_library))
        .route("/libraries/:id", delete(libraries::delete_library))
        .route("/admin/tokens", get(tokens::list_tokens).post(tokens::create_token))
        .route("/admin/tokens/:id", delete(tokens::revoke_token))
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_admin));

    let app = Router::new()
        .route("/health", get(health))
        .merge(protected)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "api listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}
