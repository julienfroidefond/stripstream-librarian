use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{atomic::AtomicU64, Arc};
use std::time::{Duration, Instant};

use lru::LruCache;
use sqlx::{Pool, Postgres};
use stripstream_core::paths::LibraryRoot;
use stripstream_core::settings::load_setting;
use tokio::sync::{AcquireError, Mutex, OwnedMutexGuard, OwnedSemaphorePermit, RwLock, Semaphore};

use crate::downloads::settings::DownloadSettings;
use crate::downloads::telegram_monitor::PendingAuth;
use crate::downloads::worker::DownloadSlots;
use crate::settings::limits::Limits;
use crate::stats::StatsCache;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    /// Mount point of the `/libraries` database prefix; built once from config.
    pub library_root: LibraryRoot,
    /// Process start, used by the diagnostics page to report uptime.
    pub started_at: Instant,
    pub bootstrap_token: Arc<str>,
    pub page_cache: Arc<Mutex<PageCache>>,
    /// Latest disk-cache walk, reused by the Backoffice's frequent cache-stat polling.
    pub disk_cache_stats: Arc<Mutex<Option<DiskCacheStatsSnapshot>>>,
    /// Coalesces concurrent cache misses for the same rendered page.
    pub page_render_locks: Arc<PageRenderLocks>,
    /// Bounds concurrent page renders to `limits.concurrent_renders`; resized
    /// in place when that setting is saved.
    pub page_render_limit: Arc<RenderLimit>,
    pub metrics: Arc<Metrics>,
    /// In-memory sliding-window limiter. A plain (non-async) mutex is used
    /// because the critical section never awaits, avoiding serialisation on
    /// the async runtime's timer wheel.
    pub read_rate_limit: Arc<std::sync::Mutex<ReadRateLimit>>,
    pub settings: Arc<RwLock<DynamicSettings>>,
    /// TTL-cached dashboard aggregates; see [`crate::stats`].
    pub stats_cache: Arc<StatsCache>,
    /// Prevents concurrent Prowlarr discovery fetches from hammering indexers simultaneously
    pub prowlarr_fetch_lock: Arc<Mutex<()>>,
    /// Holds the in-progress Telegram MTProto auth state between send-code and verify-code calls
    pub pending_tg_auth: Arc<Mutex<Option<PendingAuth>>>,
    /// Telegram file downloads: concurrency limit + cancellable abort handles
    pub telegram_downloads: DownloadSlots,
    /// DDL-provider HTTP downloads: concurrency limit + cancellable abort handles
    pub ddl_provider_downloads: DownloadSlots,
    /// Transfer tuning read from `app_settings.limits`, refreshed whenever the
    /// Backoffice saves that key so new downloads pick the change up at once.
    pub download_settings: Arc<RwLock<DownloadSettings>>,
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
        let limits = Limits::default();
        Self {
            rate_limit_per_second: limits.rate_limit_per_second.get(),
            timeout_seconds: limits.timeout_seconds.get(),
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

impl DynamicSettings {
    /// Copies the request-path knobs of the `limits` row.
    pub fn apply_limits(&mut self, limits: &Limits) {
        self.rate_limit_per_second = limits.rate_limit_per_second.get();
        self.timeout_seconds = limits.timeout_seconds.get();
    }
}

/// A page-render concurrency limit that can be resized while renders run.
///
/// It wraps a `tokio` [`Semaphore`] so waiters keep its fair FIFO queue.
/// Growing adds permits at once. Shrinking forgets the idle permits and records
/// the rest as a debt: the next permits released by running renders are
/// forgotten instead of returned, so no render is interrupted and the new
/// ceiling holds as soon as enough of them finish. A later growth repays the
/// debt first.
pub struct RenderLimit {
    semaphore: Arc<Semaphore>,
    sizing: std::sync::Mutex<RenderSizing>,
}

struct RenderSizing {
    /// The configured ceiling.
    limit: usize,
    /// Permits still owed by running renders after a shrink.
    debt: usize,
}

impl RenderLimit {
    pub fn new(limit: NonZeroUsize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limit.get())),
            sizing: std::sync::Mutex::new(RenderSizing {
                limit: limit.get(),
                debt: 0,
            }),
        }
    }

    fn sizing(&self) -> std::sync::MutexGuard<'_, RenderSizing> {
        // The critical sections never panic; recover the data if one ever did.
        self.sizing.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The configured ceiling.
    pub fn limit(&self) -> usize {
        self.sizing().limit
    }

    /// Renders currently holding a permit. Right after a shrink it may exceed
    /// [`Self::limit`] until enough running renders finish.
    pub fn in_flight(&self) -> usize {
        let sizing = self.sizing();
        (sizing.limit + sizing.debt).saturating_sub(self.semaphore.available_permits())
    }

    /// Applies a new ceiling without interrupting running renders.
    pub fn set_limit(&self, limit: NonZeroUsize) {
        let limit = limit.get();
        let mut sizing = self.sizing();
        if limit > sizing.limit {
            let grow = limit - sizing.limit;
            let repaid = grow.min(sizing.debt);
            sizing.debt -= repaid;
            self.semaphore.add_permits(grow - repaid);
        } else if limit < sizing.limit {
            let shrink = sizing.limit - limit;
            let forgotten = self.semaphore.forget_permits(shrink);
            sizing.debt += shrink - forgotten;
        }
        sizing.limit = limit;
    }

    /// Waits for a render slot under the current ceiling.
    pub async fn acquire_owned(self: Arc<Self>) -> Result<RenderPermit, AcquireError> {
        let permit = self.semaphore.clone().acquire_owned().await?;
        Ok(RenderPermit {
            permit: Some(permit),
            limit: self,
        })
    }
}

/// One render slot of a [`RenderLimit`], released on drop.
pub struct RenderPermit {
    permit: Option<OwnedSemaphorePermit>,
    limit: Arc<RenderLimit>,
}

impl Drop for RenderPermit {
    fn drop(&mut self) {
        let Some(permit) = self.permit.take() else {
            return;
        };
        // Decide and release under the sizing lock so `set_limit` always sees
        // a consistent count of available permits.
        let mut sizing = self.limit.sizing();
        if sizing.debt > 0 {
            sizing.debt -= 1;
            permit.forget();
        } else {
            drop(permit);
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

/// Minimal [`AppState`] for unit and DB tests: all fields are present but only
/// `pool` carries meaningful state. Shared by every test module so adding a
/// field to `AppState` touches a single place.
#[cfg(test)]
pub(crate) fn test_state(pool: Pool<Postgres>) -> AppState {
    AppState {
        pool,
        library_root: LibraryRoot::new("/libraries"),
        started_at: Instant::now(),
        bootstrap_token: Arc::from("test-token"),
        page_cache: Arc::new(Mutex::new(PageCache::new(1))),
        disk_cache_stats: Arc::new(Mutex::new(None)),
        page_render_locks: Arc::new(PageRenderLocks::new(1)),
        page_render_limit: Arc::new(RenderLimit::new(NonZeroUsize::MIN)),
        metrics: Arc::new(Metrics::new()),
        read_rate_limit: Arc::new(std::sync::Mutex::new(ReadRateLimit::new())),
        settings: Arc::new(RwLock::new(DynamicSettings::default())),
        stats_cache: Arc::new(StatsCache::new()),
        prowlarr_fetch_lock: Arc::new(Mutex::new(())),
        pending_tg_auth: Arc::new(Mutex::new(None)),
        telegram_downloads: DownloadSlots::new(1, Arc::new(Mutex::new(HashMap::new()))),
        ddl_provider_downloads: DownloadSlots::new(1, Arc::new(Mutex::new(HashMap::new()))),
        download_settings: Arc::new(RwLock::new(DownloadSettings::default())),
    }
}

pub async fn load_dynamic_settings(pool: &Pool<Postgres>) -> DynamicSettings {
    let mut s = DynamicSettings::default();
    s.apply_limits(&Limits::load(pool).await);

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
    use super::{PageCache, PageRenderLocks, RenderLimit};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::{sync::oneshot, time::timeout};

    fn nz(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    async fn blocked(limit: &Arc<RenderLimit>) -> bool {
        timeout(Duration::from_millis(30), limit.clone().acquire_owned())
            .await
            .is_err()
    }

    #[tokio::test]
    async fn render_limit_grows_at_once() {
        let limit = Arc::new(RenderLimit::new(nz(1)));
        let _first = limit.clone().acquire_owned().await.unwrap();
        assert!(blocked(&limit).await);

        limit.set_limit(nz(2));
        let _second = limit.clone().acquire_owned().await.unwrap();
        assert_eq!(limit.limit(), 2);
        assert_eq!(limit.in_flight(), 2);
        assert!(blocked(&limit).await);
    }

    #[tokio::test]
    async fn render_limit_shrinks_without_interrupting_running_renders() {
        let limit = Arc::new(RenderLimit::new(nz(3)));
        let a = limit.clone().acquire_owned().await.unwrap();
        let b = limit.clone().acquire_owned().await.unwrap();

        // The idle permit is forgotten at once, one more is owed by a running render.
        limit.set_limit(nz(1));
        assert_eq!(limit.limit(), 1);
        assert_eq!(limit.in_flight(), 2);
        assert!(blocked(&limit).await);

        // The first release repays the debt: still at the new ceiling of 1.
        drop(a);
        assert_eq!(limit.in_flight(), 1);
        assert!(blocked(&limit).await);

        drop(b);
        assert_eq!(limit.in_flight(), 0);
        let _only = limit.clone().acquire_owned().await.unwrap();
        assert!(blocked(&limit).await);
    }

    #[tokio::test]
    async fn render_limit_growth_repays_a_pending_shrink_first() {
        let limit = Arc::new(RenderLimit::new(nz(2)));
        let a = limit.clone().acquire_owned().await.unwrap();
        let b = limit.clone().acquire_owned().await.unwrap();

        limit.set_limit(nz(1)); // one permit owed
        limit.set_limit(nz(2)); // the debt is cancelled, no extra permit
        assert_eq!(limit.in_flight(), 2);
        assert!(blocked(&limit).await);

        drop(a);
        drop(b);
        let _x = limit.clone().acquire_owned().await.unwrap();
        let _y = limit.clone().acquire_owned().await.unwrap();
        assert!(blocked(&limit).await);
    }

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
