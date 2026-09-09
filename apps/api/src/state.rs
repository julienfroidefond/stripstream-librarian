use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};
use std::time::Instant;

use lru::LruCache;
use sqlx::{Pool, Postgres, Row};
use tokio::sync::{Mutex, RwLock, Semaphore};
use uuid::Uuid;

use crate::downloads::telegram_monitor::PendingAuth;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub bootstrap_token: Arc<str>,
    pub page_cache: Arc<Mutex<PageCache>>,
    pub page_render_limit: Arc<Semaphore>,
    pub metrics: Arc<Metrics>,
    pub read_rate_limit: Arc<Mutex<ReadRateLimit>>,
    pub settings: Arc<RwLock<DynamicSettings>>,
    /// Prevents concurrent Prowlarr discovery fetches from hammering indexers simultaneously
    pub prowlarr_fetch_lock: Arc<Mutex<()>>,
    /// Holds the in-progress Telegram MTProto auth state between send-code and verify-code calls
    pub pending_tg_auth: Arc<Mutex<Option<PendingAuth>>>,
    /// Limits concurrent Telegram file downloads to avoid FLOOD_WAIT errors
    pub telegram_download_limit: Arc<Semaphore>,
    /// Abort handles for active/queued Telegram downloads, keyed by book link ID
    pub telegram_abort_handles: Arc<Mutex<HashMap<Uuid, tokio::task::AbortHandle>>>,
}

#[derive(Clone)]
pub struct DynamicSettings {
    pub rate_limit_per_second: u32,
    pub timeout_seconds: u64,
    pub image_format: String,
    pub image_quality: u8,
    pub image_filter: String,
    pub image_max_width: u32,
    pub cache_directory: String,
    pub page_cache_max_size_mb: usize,
}

impl Default for DynamicSettings {
    fn default() -> Self {
        Self {
            rate_limit_per_second: 120,
            timeout_seconds: 12,
            image_format: "webp".to_string(),
            image_quality: 85,
            image_filter: "triangle".to_string(),
            image_max_width: 2160,
            cache_directory: std::env::var("IMAGE_CACHE_DIR")
                .unwrap_or_else(|_| "/tmp/stripstream-image-cache".to_string()),
            page_cache_max_size_mb: 128,
        }
    }
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

pub async fn load_concurrent_telegram_downloads(pool: &Pool<Postgres>) -> usize {
    let default_concurrency = 2;
    let row = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'limits'"#)
        .fetch_optional(pool)
        .await;

    match row {
        Ok(Some(row)) => {
            let value: serde_json::Value = row.get("value");
            value
                .get("concurrent_telegram_downloads")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .map(|v| (v as usize).max(1))
                .unwrap_or(default_concurrency)
        }
        _ => default_concurrency,
    }
}

pub async fn load_dynamic_settings(pool: &Pool<Postgres>) -> DynamicSettings {
    let mut s = DynamicSettings::default();

    if let Ok(Some(row)) = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'limits'"#)
        .fetch_optional(pool)
        .await
    {
        let v: serde_json::Value = row.get("value");
        if let Some(n) = v.get("rate_limit_per_second").and_then(|x| x.as_u64()) {
            s.rate_limit_per_second = n as u32;
        }
        if let Some(n) = v.get("timeout_seconds").and_then(|x| x.as_u64()) {
            s.timeout_seconds = n;
        }
    }

    if let Ok(Some(row)) =
        sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'image_processing'"#)
            .fetch_optional(pool)
            .await
    {
        let v: serde_json::Value = row.get("value");
        if let Some(s2) = v.get("format").and_then(|x| x.as_str()) {
            s.image_format = s2.to_string();
        }
        if let Some(n) = v.get("quality").and_then(|x| x.as_u64()) {
            s.image_quality = n.clamp(1, 100) as u8;
        }
        if let Some(s2) = v.get("filter").and_then(|x| x.as_str()) {
            s.image_filter = s2.to_string();
        }
        if let Some(n) = v.get("max_width").and_then(|x| x.as_u64()) {
            s.image_max_width = n as u32;
        }
    }

    if let Ok(Some(row)) = sqlx::query(r#"SELECT value FROM app_settings WHERE key = 'cache'"#)
        .fetch_optional(pool)
        .await
    {
        let v: serde_json::Value = row.get("value");
        if let Some(dir) = v.get("directory").and_then(|x| x.as_str()) {
            s.cache_directory = dir.to_string();
        }
        if let Some(n) = v.get("memory_max_size_mb").and_then(|x| x.as_u64()) {
            s.page_cache_max_size_mb = (n as usize).max(1);
        }
    }

    s
}

pub struct PageCache {
    entries: LruCache<String, Arc<Vec<u8>>>,
    current_size_bytes: usize,
    max_size_bytes: usize,
}

impl PageCache {
    pub fn new(max_size_mb: usize) -> Self {
        let max_size_bytes = max_size_mb.max(1).saturating_mul(1024 * 1024);
        Self {
            entries: LruCache::unbounded(),
            current_size_bytes: 0,
            max_size_bytes,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&Arc<Vec<u8>>> {
        self.entries.get(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains(key)
    }

    pub fn put(&mut self, key: String, value: Arc<Vec<u8>>) {
        let value_size = value.len();
        if let Some(previous) = self.entries.put(key, value) {
            self.current_size_bytes = self.current_size_bytes.saturating_sub(previous.len());
        }
        self.current_size_bytes = self.current_size_bytes.saturating_add(value_size);
        self.evict_to_limit();
    }

    pub fn set_max_size_mb(&mut self, max_size_mb: usize) {
        self.max_size_bytes = max_size_mb.max(1).saturating_mul(1024 * 1024);
        self.evict_to_limit();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size_bytes = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn current_size_bytes(&self) -> usize {
        self.current_size_bytes
    }

    pub fn max_size_bytes(&self) -> usize {
        self.max_size_bytes
    }

    fn evict_to_limit(&mut self) {
        while self.current_size_bytes > self.max_size_bytes {
            let Some((_key, value)) = self.entries.pop_lru() else {
                self.current_size_bytes = 0;
                break;
            };
            self.current_size_bytes = self.current_size_bytes.saturating_sub(value.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PageCache;
    use std::sync::Arc;

    #[test]
    fn page_cache_evicts_lru_entries_to_respect_byte_limit() {
        let mut cache = PageCache::new(2);
        cache.put("first".into(), Arc::new(vec![0; 1024 * 1024]));
        cache.put("second".into(), Arc::new(vec![0; 1024 * 1024]));
        let _ = cache.get("first");
        cache.put("third".into(), Arc::new(vec![0; 1024 * 1024]));

        assert!(cache.contains("first"));
        assert!(!cache.contains("second"));
        assert!(cache.contains("third"));
    }

    #[test]
    fn lowering_page_cache_limit_evicts_immediately() {
        let mut cache = PageCache::new(3);
        cache.put("first".into(), Arc::new(vec![0; 1024 * 1024]));
        cache.put("second".into(), Arc::new(vec![0; 1024 * 1024]));

        cache.set_max_size_mb(1);

        assert!(!cache.contains("first"));
        assert!(cache.contains("second"));
    }
}
