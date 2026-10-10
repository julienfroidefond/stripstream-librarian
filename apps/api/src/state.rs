use std::collections::HashMap;
use std::sync::{atomic::AtomicU64, Arc};
use std::time::{Duration, Instant};

use lru::LruCache;
use sqlx::{Pool, Postgres};
use stripstream_core::settings::load_setting;
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock, Semaphore};
use uuid::Uuid;

use crate::downloads::telegram_monitor::PendingAuth;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub bootstrap_token: Arc<str>,
    pub page_cache: Arc<Mutex<PageCache>>,
    /// Latest disk-cache walk, reused by the Backoffice's frequent cache-stat polling.
    pub disk_cache_stats: Arc<Mutex<Option<DiskCacheStatsSnapshot>>>,
    /// Coalesces concurrent cache misses for the same rendered page.
    pub page_render_locks: Arc<PageRenderLocks>,
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
pub struct DiskCacheStatsSnapshot {
    pub directory: String,
    pub total_size_bytes: u64,
    pub file_count: u64,
    pub collected_at: Instant,
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

#[derive(Default)]
pub struct ReadRateLimit {
    windows: HashMap<String, RateWindow>,
    last_sweep: Option<Instant>,
}

struct RateWindow {
    started_at: Instant,
    requests: u32,
}

impl ReadRateLimit {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when the caller may proceed, advancing its 1-second window.
    pub fn check(&mut self, key: &str, limit: u32) -> bool {
        let now = Instant::now();
        self.sweep(now);

        let window = self.windows.entry(key.to_string()).or_insert(RateWindow {
            started_at: now,
            requests: 0,
        });

        if now.duration_since(window.started_at) >= Duration::from_secs(1) {
            window.started_at = now;
            window.requests = 0;
        }

        if window.requests >= limit {
            return false;
        }
        window.requests += 1;
        true
    }

    fn sweep(&mut self, now: Instant) {
        let due = match self.last_sweep {
            None => true,
            Some(last) => now.duration_since(last) >= Duration::from_secs(60),
        };
        if !due {
            return;
        }
        self.windows
            .retain(|_, window| now.duration_since(window.started_at) < Duration::from_secs(60));
        self.last_sweep = Some(now);
    }

    #[cfg(test)]
    pub fn expire_all(&mut self, age: Duration) {
        let now = Instant::now();
        for window in self.windows.values_mut() {
            window.started_at = now - age;
        }
    }
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
    load_setting::<serde_json::Value>(pool, "limits")
        .await
        .ok()
        .flatten()
        .and_then(|value| value.get("concurrent_renders").and_then(|v| v.as_u64()))
        // Clamp to >= 1: `Semaphore::new(0)` would block every page render forever.
        .map(|v| (v as usize).max(1))
        .unwrap_or(default_concurrency)
}

pub async fn load_concurrent_telegram_downloads(pool: &Pool<Postgres>) -> usize {
    let default_concurrency = 2;
    load_setting::<serde_json::Value>(pool, "limits")
        .await
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("concurrent_telegram_downloads")
                .and_then(|v| v.as_u64())
        })
        .map(|v| (v as usize).max(1))
        .unwrap_or(default_concurrency)
}

pub async fn load_dynamic_settings(pool: &Pool<Postgres>) -> DynamicSettings {
    let mut s = DynamicSettings::default();

    if let Some(v) = load_setting::<serde_json::Value>(pool, "limits")
        .await
        .ok()
        .flatten()
    {
        if let Some(n) = v.get("rate_limit_per_second").and_then(|x| x.as_u64()) {
            // A rate limit of 0 would reject every read request (429) permanently.
            s.rate_limit_per_second = (n as u32).max(1);
        }
        if let Some(n) = v.get("timeout_seconds").and_then(|x| x.as_u64()) {
            s.timeout_seconds = n.max(1);
        }
    }

    if let Some(v) = load_setting::<serde_json::Value>(pool, "image_processing")
        .await
        .ok()
        .flatten()
    {
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
            s.image_max_width = (n as u32).max(1);
        }
    }

    if let Some(v) = load_setting::<serde_json::Value>(pool, "cache")
        .await
        .ok()
        .flatten()
    {
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
    entries: LruCache<String, Arc<[u8]>>,
    current_size_bytes: usize,
    max_size_bytes: usize,
}

/// A bounded key-to-lock registry for page rendering. It prevents a foreground request and
/// prefetches (or several readers) from rendering the exact same cache key concurrently.
pub struct PageRenderLocks {
    entries: Mutex<LruCache<String, Arc<Mutex<()>>>>,
    max_entries: usize,
}

impl PageRenderLocks {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(LruCache::unbounded()),
            max_entries: max_entries.max(1),
        }
    }

    pub async fn acquire(&self, key: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut entries = self.entries.lock().await;
            let lock = entries.get(key).cloned().unwrap_or_else(|| {
                let lock = Arc::new(Mutex::new(()));
                entries.put(key.to_owned(), lock.clone());
                lock
            });
            while entries.len() > self.max_entries {
                entries.pop_lru();
            }
            lock
        };
        lock.lock_owned().await
    }
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

    pub fn get(&mut self, key: &str) -> Option<&Arc<[u8]>> {
        self.entries.get(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains(key)
    }

    pub fn put(&mut self, key: String, value: Arc<[u8]>) {
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
    use super::{PageCache, PageRenderLocks};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::{sync::oneshot, time::timeout};

    #[test]
    fn page_cache_evicts_lru_entries_to_respect_byte_limit() {
        let mut cache = PageCache::new(2);
        cache.put("first".into(), Arc::from(vec![0; 1024 * 1024]));
        cache.put("second".into(), Arc::from(vec![0; 1024 * 1024]));
        let _ = cache.get("first");
        cache.put("third".into(), Arc::from(vec![0; 1024 * 1024]));

        assert!(cache.contains("first"));
        assert!(!cache.contains("second"));
        assert!(cache.contains("third"));
    }

    #[test]
    fn lowering_page_cache_limit_evicts_immediately() {
        let mut cache = PageCache::new(3);
        cache.put("first".into(), Arc::from(vec![0; 1024 * 1024]));
        cache.put("second".into(), Arc::from(vec![0; 1024 * 1024]));

        cache.set_max_size_mb(1);

        assert!(!cache.contains("first"));
        assert!(cache.contains("second"));
    }

    #[tokio::test]
    async fn page_render_locks_serialize_the_same_cache_key() {
        let locks = Arc::new(PageRenderLocks::new(2));
        let first = locks.acquire("page-key").await;
        let (sent, mut received) = oneshot::channel();
        let waiting_locks = locks.clone();

        tokio::spawn(async move {
            let _second = waiting_locks.acquire("page-key").await;
            let _ = sent.send(());
        });

        assert!(timeout(Duration::from_millis(20), &mut received)
            .await
            .is_err());
        drop(first);
        assert!(timeout(Duration::from_secs(1), received).await.is_ok());
    }
}
