pub mod analyzer;
pub mod api;
pub mod batch;
pub mod converter;
pub mod job;
pub mod scanner;
pub mod scheduler;
pub mod utils;
pub mod watcher;
pub mod worker;

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}
