use std::sync::{
    atomic::AtomicU64,
    Arc,
};
use std::time::Instant;

use lru::LruCache;
use sqlx::{Pool, Postgres, Row};
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub bootstrap_token: Arc<str>,
    pub meili_url: Arc<str>,
    pub meili_master_key: Arc<str>,
    pub page_cache: Arc<Mutex<LruCache<String, Arc<Vec<u8>>>>>,
    pub page_render_limit: Arc<Semaphore>,
    pub metrics: Arc<Metrics>,
    pub read_rate_limit: Arc<Mutex<ReadRateLimit>>,
}

pub struct Metrics {
    pub requests_total: AtomicU64,
    pub page_cache_hits: AtomicU64,
    pub page_cache_misses: AtomicU64,
}

pub struct ReadRateLimit {
    pub window_started_at: Instant,
    pub requests_in_window: u32,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            page_cache_hits: AtomicU64::new(0),
            page_cache_misses: AtomicU64::new(0),
        }
    }
}

pub async fn load_concurrent_renders(pool: &Pool<Postgres>) -> usize {
    let default_concurrency = 8;
    let row = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'limits'"#)
        .fetch_optional(pool)
        .await;

    match row {
        Ok(Some(row)) => {
            let value: serde_json::Value = row.get("value");
            value
                .get("concurrent_renders")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(default_concurrency)
        }
        _ => default_concurrency,
    }
}
