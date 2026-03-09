pub mod analyzer;
pub mod api;
pub mod batch;
pub mod job;
pub mod meili;
pub mod scheduler;
pub mod scanner;
pub mod utils;
pub mod watcher;
pub mod worker;

use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub meili_url: String,
    pub meili_master_key: String,
}
