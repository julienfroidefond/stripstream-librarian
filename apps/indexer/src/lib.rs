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
use stripstream_core::paths::LibraryRoot;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// Mount point of the `/libraries` database prefix; built once from config.
    pub library_root: LibraryRoot,
}

/// Minimal [`AppState`] for unit and DB tests.
#[cfg(test)]
pub(crate) fn test_state(pool: PgPool) -> AppState {
    AppState {
        pool,
        library_root: LibraryRoot::new(stripstream_core::paths::LIBRARIES_DB_ROOT),
    }
}
