//! A concurrency limit that can be changed while tasks are running.
//!
//! `tokio::sync::Semaphore` cannot shrink (and rebuilding it would strand every
//! task already queued on the old instance), so the running count is tracked
//! explicitly: waiters park on a [`Notify`] and re-check the current limit when
//! woken. Lowering the limit never interrupts a running task — it only stops
//! admitting new ones until the count drops below the new value.

use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::Notify;

/// A runtime-adjustable concurrency limit.
pub struct DynamicLimit {
    limit: AtomicUsize,
    in_use: AtomicUsize,
    notify: Notify,
}

impl DynamicLimit {
    /// Builds a limit that admits `limit` concurrent holders. Zero is clamped to
    /// one: a limit of zero would block the queue forever.
    pub const fn new(limit: usize) -> Self {
        Self {
            limit: AtomicUsize::new(if limit == 0 { 1 } else { limit }),
            in_use: AtomicUsize::new(0),
            notify: Notify::const_new(),
        }
    }

    /// The currently configured limit.
    pub fn limit(&self) -> usize {
        self.limit.load(Ordering::Acquire)
    }

    /// The number of permits currently held.
    #[cfg(test)]
    pub fn in_use(&self) -> usize {
        self.in_use.load(Ordering::Acquire)
    }

    /// Applies a new limit. Raising it wakes the waiters immediately; lowering it
    /// only takes effect as running holders release their permits.
    pub fn set_limit(&self, limit: usize) {
        let previous = self.limit.swap(limit.max(1), Ordering::AcqRel);
        if limit.max(1) > previous {
            self.notify.notify_waiters();
        }
    }

    /// Waits for a free slot under the current limit.
    pub async fn acquire(&self) -> DynamicLimitPermit<'_> {
        loop {
            // Register interest *before* checking, otherwise a release between
            // the check and the await would be missed and the task would sleep
            // until the next unrelated notification.
            let notified = self.notify.notified();
            let mut notified = std::pin::pin!(notified);
            notified.as_mut().enable();

            if let Some(permit) = self.try_acquire() {
                return permit;
            }
            notified.await;
        }
    }

    /// Takes a slot without waiting, for callers that can proceed with a smaller
    /// share of the budget.
    pub fn try_acquire(&self) -> Option<DynamicLimitPermit<'_>> {
        let limit = self.limit.load(Ordering::Acquire);
        let mut in_use = self.in_use.load(Ordering::Acquire);
        loop {
            if in_use >= limit {
                return None;
            }
            match self.in_use.compare_exchange_weak(
                in_use,
                in_use + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(DynamicLimitPermit { limit: self }),
                Err(actual) => in_use = actual,
            }
        }
    }
}

/// Holds one slot of a [`DynamicLimit`] for as long as it is alive.
pub struct DynamicLimitPermit<'a> {
    limit: &'a DynamicLimit,
}

impl Drop for DynamicLimitPermit<'_> {
    fn drop(&mut self) {
        self.limit.in_use.fetch_sub(1, Ordering::AcqRel);
        self.limit.notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    async fn blocked(limit: &DynamicLimit) -> bool {
        tokio::time::timeout(Duration::from_millis(50), limit.acquire())
            .await
            .is_err()
    }

    #[tokio::test]
    async fn admits_exactly_the_configured_number_of_holders() {
        let limit = DynamicLimit::new(2);
        let _first = limit.acquire().await;
        let _second = limit.acquire().await;

        assert_eq!(limit.in_use(), 2);
        assert!(blocked(&limit).await);
    }

    #[tokio::test]
    async fn releasing_a_slot_wakes_a_waiter() {
        let limit = DynamicLimit::new(1);
        let first = limit.acquire().await;
        assert!(blocked(&limit).await);

        drop(first);
        tokio::time::timeout(Duration::from_millis(50), limit.acquire())
            .await
            .expect("releasing the only permit must wake the waiter");
    }

    #[tokio::test]
    async fn raising_the_limit_admits_waiters_immediately() {
        let limit = DynamicLimit::new(1);
        let _first = limit.acquire().await;
        assert!(blocked(&limit).await);

        limit.set_limit(2);
        tokio::time::timeout(Duration::from_millis(50), limit.acquire())
            .await
            .expect("raising the limit must wake the waiter");
    }

    #[tokio::test]
    async fn lowering_the_limit_waits_for_running_holders() {
        let limit = DynamicLimit::new(2);
        let first = limit.acquire().await;
        let second = limit.acquire().await;

        limit.set_limit(1);
        assert_eq!(limit.in_use(), 2, "running holders are never interrupted");
        assert!(blocked(&limit).await);

        drop(first);
        assert!(
            blocked(&limit).await,
            "still at the (new) limit while one holder remains"
        );

        drop(second);
        tokio::time::timeout(Duration::from_millis(50), limit.acquire())
            .await
            .expect("a freed slot must be admitted again");
    }

    #[test]
    fn zero_is_clamped_to_one() {
        assert_eq!(DynamicLimit::new(0).limit(), 1);
        let limit = DynamicLimit::new(2);
        limit.set_limit(0);
        assert_eq!(limit.limit(), 1);
    }

    #[tokio::test]
    async fn try_acquire_never_exceeds_the_limit() {
        let limit = DynamicLimit::new(1);
        let _first = limit.try_acquire().expect("first slot is free");
        assert!(limit.try_acquire().is_none());
    }
}
