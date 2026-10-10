//! Small in-process TTL cache for expensive, read-mostly responses.
//!
//! Some endpoints (the dashboard aggregates) run several heavy scans over the
//! whole `books`/`book_files` tables. The Backoffice polls them on a short
//! interval, so recomputing on every request wastes database work and can
//! monopolise the connection pool. Caching the built response for a few seconds
//! absorbs that polling without making indexer updates visibly stale.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// A tiny generic TTL cache keyed by `K`, storing shared `Arc<V>` values.
///
/// Lookups and inserts never await, so a plain [`std::sync::Mutex`] is enough
/// and avoids the overhead of an async lock on the request hot path.
pub struct TtlCache<K, V> {
    ttl: Duration,
    entries: Mutex<HashMap<K, (Instant, Arc<V>)>>,
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Return the cached value when it is still within the TTL, else `None`.
    pub fn get(&self, key: &K) -> Option<Arc<V>> {
        let entries = self.lock();
        let (stored_at, value) = entries.get(key)?;
        (stored_at.elapsed() < self.ttl).then(|| Arc::clone(value))
    }

    /// Store (or replace) the value for `key` and return a shared handle to it.
    ///
    /// Expired entries are pruned opportunistically so the map cannot grow
    /// without bound when many distinct keys (e.g. users) are seen.
    pub fn insert(&self, key: K, value: V) -> Arc<V> {
        let value = Arc::new(value);
        let mut entries = self.lock();
        if !entries.is_empty() {
            entries.retain(|_, (stored_at, _)| stored_at.elapsed() < self.ttl);
        }
        entries.insert(key, (Instant::now(), Arc::clone(&value)));
        value
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.lock().clear();
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<K, (Instant, Arc<V>)>> {
        // A panic while holding this lock only affects a cache, so recover the
        // guard rather than poisoning every subsequent request.
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::TtlCache;
    use std::time::Duration;

    #[test]
    fn returns_value_within_ttl() {
        let cache: TtlCache<&str, u32> = TtlCache::new(Duration::from_secs(60));
        cache.insert("a", 1);

        assert_eq!(*cache.get(&"a").unwrap(), 1);
    }

    #[test]
    fn misses_unknown_key() {
        let cache: TtlCache<&str, u32> = TtlCache::new(Duration::from_secs(60));

        assert!(cache.get(&"missing").is_none());
    }

    #[test]
    fn expires_after_ttl() {
        let cache: TtlCache<&str, u32> = TtlCache::new(Duration::from_millis(1));
        cache.insert("a", 1);

        std::thread::sleep(Duration::from_millis(5));

        assert!(cache.get(&"a").is_none());
    }

    #[test]
    fn insert_returns_shared_handle() {
        let cache: TtlCache<&str, u32> = TtlCache::new(Duration::from_secs(60));
        let stored = cache.insert("a", 7);

        assert_eq!(*stored, 7);
        assert_eq!(*cache.get(&"a").unwrap(), 7);
    }

    #[test]
    fn clear_empties_cache() {
        let cache: TtlCache<&str, u32> = TtlCache::new(Duration::from_secs(60));
        cache.insert("a", 1);
        cache.clear();

        assert!(cache.get(&"a").is_none());
    }
}
