//! Process-wide coordination for Telegram rate limits.
//!
//! Telegram throttles an **account**, not an individual connection. When one
//! `upload.getFile` request is answered with `420 FLOOD_WAIT`, every other
//! request sent during the wait can extend it. Independent downloads each
//! opening their own connections therefore used to multiply the request rate
//! until Telegram flooded the whole session — the more downloads running, the
//! worse it got.
//!
//! This module centralises both sides of the contract:
//!
//! * [`FloodGate`] pauses **every** Telegram call for the advertised delay, so
//!   no connection keeps hammering while another one sleeps;
//! * the file-request budget caps the number of in-flight `upload.getFile`
//!   requests across all downloads, so starting a second download shares the
//!   budget instead of doubling the pressure. A `FLOOD_WAIT` shrinks that
//!   budget (down to a single request) and it grows back one slot at a time
//!   once Telegram has stayed quiet for a while;
//! * the connection budget caps the number of sockets opened for downloads, so
//!   a third and fourth simultaneous download share the existing connections
//!   instead of multiplying `initConnection` bursts.

use std::sync::Mutex;
use std::time::Duration;

use grammers_client::client::auth::AuthorizationError;
use grammers_client::InvocationError;
use tokio::time::Instant;

use crate::downloads::limit::{DynamicLimit, DynamicLimitPermit};

/// Maximum concurrent `upload.getFile` requests for the whole process.
///
/// `telegram_download_connections` (default 4) × `workers_per_fetcher` (2)
/// already reaches this cap with a single download. Keeping the cap global is
/// what makes two simultaneous downloads share it instead of doubling it.
pub const MAX_CONCURRENT_FILE_REQUESTS: usize = 8;

/// Maximum Telegram connections kept open for downloads by the whole process.
///
/// Every download asks for `telegram_download_connections` sockets, but they
/// come out of this shared budget: past two concurrent downloads each one gets
/// fewer sockets instead of the account accumulating more logins. Telegram
/// throttles the account, not a single connection, so multiplying sockets is
/// what used to trigger `FLOOD_WAIT` as soon as a third download started.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 8;

/// A successful request must be this far from the last flood before one extra
/// in-flight request slot is restored.
const REQUEST_RECOVERY_INTERVAL: Duration = Duration::from_secs(15);

/// A burst of simultaneous 420s comes from the same flood window and must not
/// collapse the request budget straight to 1; only a later window shrinks it
/// again.
const SHRINK_COOLDOWN: Duration = Duration::from_secs(2);

/// Longest cooldown accepted from Telegram. A `u32` flood value can span
/// decades; clamping keeps deadline arithmetic safe.
pub const MAX_FLOOD_WAIT: Duration = Duration::from_secs(24 * 60 * 60);

/// Coordinates Telegram requests inside the API process.
pub struct FloodGate {
    /// Instant until which every Telegram call must wait.
    until: Mutex<Option<Instant>>,
    /// In-flight `upload.getFile` budget, shrunk after a flood wait.
    file_requests: DynamicLimit,
    /// Budget the file requests return to after a calm period.
    max_file_requests: usize,
    /// When the last flood wait was observed; `None` until the first one.
    last_flood: Mutex<Option<Instant>>,
    /// Download sockets shared by every running download.
    connections: DynamicLimit,
}

impl FloodGate {
    const fn new() -> Self {
        Self {
            until: Mutex::new(None),
            file_requests: DynamicLimit::new(MAX_CONCURRENT_FILE_REQUESTS),
            max_file_requests: MAX_CONCURRENT_FILE_REQUESTS,
            last_flood: Mutex::new(None),
            connections: DynamicLimit::new(MAX_CONCURRENT_CONNECTIONS),
        }
    }

    /// Blocks until the current flood-wait window (if any) has elapsed.
    pub async fn wait(&self) {
        loop {
            let remaining = {
                let until = self.until.lock().unwrap_or_else(|err| err.into_inner());
                until.and_then(|deadline| deadline.checked_duration_since(Instant::now()))
            };
            match remaining {
                Some(delay) => tokio::time::sleep(delay).await,
                None => return,
            }
        }
    }

    /// Extends the cooldown to at least `delay` from now. Never shortens an
    /// existing window: a later, shorter flood wait must not resume traffic
    /// early.
    pub async fn defer(&self, delay: Duration) {
        let deadline = Instant::now() + delay.min(MAX_FLOOD_WAIT);
        let mut until = self.until.lock().unwrap_or_else(|err| err.into_inner());
        if until.is_none_or(|current| current < deadline) {
            *until = Some(deadline);
        }
    }

    /// Reserves one globally-available `upload.getFile` slot.
    pub async fn acquire_file_request(&self) -> DynamicLimitPermit<'_> {
        self.file_requests.acquire().await
    }

    /// Current `upload.getFile` budget, exposed for tests.
    #[cfg(test)]
    pub fn file_request_limit(&self) -> usize {
        self.file_requests.limit()
    }

    /// Reserves one slot of the process-wide download connection budget.
    pub async fn acquire_connection(&self) -> DynamicLimitPermit<'_> {
        self.connections.acquire().await
    }

    /// Reserves a connection slot only if one is free, so a download starting
    /// while others are running can settle for fewer sockets.
    pub fn try_acquire_connection(&self) -> Option<DynamicLimitPermit<'_>> {
        self.connections.try_acquire()
    }

    /// Telegram answered `FLOOD_WAIT`: pause every call for the advertised delay
    /// and narrow the in-flight request budget so the workers that wake up do
    /// not immediately provoke another flood.
    pub async fn defer_flood(&self, delay: Duration) {
        self.defer(delay).await;

        let now = Instant::now();
        let mut last_flood = self
            .last_flood
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let shrink = last_flood.is_none_or(|last| now.duration_since(last) >= SHRINK_COOLDOWN);
        *last_flood = Some(now);
        drop(last_flood);

        if !shrink {
            tracing::warn!(
                "[TG] FLOOD_WAIT: pausing every Telegram call for {}s",
                delay.as_secs()
            );
            return;
        }

        let current = self.file_requests.limit();
        let next = (current / 2).max(1);
        if next < current {
            self.file_requests.set_limit(next);
            tracing::warn!(
                "[TG] FLOOD_WAIT: pausing every Telegram call for {}s and lowering \
                 concurrent upload.getFile requests {current} -> {next}",
                delay.as_secs()
            );
        } else {
            tracing::warn!(
                "[TG] FLOOD_WAIT: pausing every Telegram call for {}s (already at 1 request)",
                delay.as_secs()
            );
        }
    }

    /// A request succeeded. If Telegram has been quiet long enough, restore one
    /// slot of the in-flight budget.
    pub fn record_success(&self) {
        self.recover(Instant::now());
    }

    fn recover(&self, now: Instant) {
        if self.file_requests.limit() >= self.max_file_requests {
            return;
        }
        let mut last_flood = self
            .last_flood
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let Some(last) = *last_flood else {
            return;
        };
        if now.duration_since(last) < REQUEST_RECOVERY_INTERVAL {
            return;
        }

        let current = self.file_requests.limit();
        self.file_requests.set_limit(current + 1);
        *last_flood = Some(now);
        tracing::info!(
            "[TG] Telegram calmed down: raising concurrent upload.getFile requests {current} -> {}",
            current + 1
        );
    }
}

static GATE: FloodGate = FloodGate::new();

/// The process-wide gate shared by every Telegram call.
pub fn gate() -> &'static FloodGate {
    &GATE
}

/// Delay Telegram asked us to wait, plus a one-second safety margin.
///
/// Returns `None` when the error is not a flood wait.
pub fn flood_wait_delay(code: i32, value: Option<u32>) -> Option<Duration> {
    if code != 420 {
        return None;
    }
    let seconds = u64::from(value.unwrap_or(5)) + 1;
    Some(Duration::from_secs(
        seconds.min(MAX_FLOOD_WAIT.as_secs().saturating_sub(1)),
    ))
}

/// Delay carried by a grammers invocation error, if it is a flood wait.
pub fn flood_delay(err: &InvocationError) -> Option<Duration> {
    match err {
        InvocationError::Rpc(rpc) => flood_wait_delay(rpc.code, rpc.value),
        _ => None,
    }
}

/// Delay carried by a connection handshake error, if it is a flood wait.
pub fn authorization_flood_delay(err: &AuthorizationError) -> Option<Duration> {
    match err {
        AuthorizationError::Invoke(inner) => flood_delay(inner),
        AuthorizationError::Gen(_) => None,
    }
}

/// Grammers parameters that surface every flood wait to [`retry_on_flood`]
/// instead of letting each connection sleep on its own.
pub fn init_params() -> grammers_client::InitParams {
    grammers_client::InitParams {
        flood_sleep_threshold: 0,
        ..Default::default()
    }
}

/// Longest total delay accepted before giving up, so a scheduled sync job
/// cannot hang forever on a permanently flooded account.
pub const MAX_TOTAL_FLOOD_RETRY: Duration = Duration::from_secs(30 * 60);

/// Runs `operation`, honouring every `420 FLOOD_WAIT` by pausing the whole
/// process before retrying.
pub async fn retry_on_flood<T, F, Fut>(mut operation: F) -> Result<T, InvocationError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, InvocationError>>,
{
    let mut waited = Duration::ZERO;
    loop {
        gate().wait().await;
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) => match flood_delay(&err) {
                Some(delay) => {
                    if waited + delay > MAX_TOTAL_FLOOD_RETRY {
                        return Err(err);
                    }
                    waited += delay;
                    gate().defer_flood(delay).await;
                }
                None => return Err(err),
            },
        }
    }
}

/// Evaluates to `Result<Option<T>, InvocationError>` for one `iter.next()`
/// call, retrying flood waits through the process-wide gate.
///
/// A macro (rather than a generic helper) is used because `next()` borrows the
/// iterator mutably: the borrow must not escape the future the closure returns,
/// which a `FnMut` helper cannot express.
macro_rules! next_on_flood {
    ($iter:expr) => {{
        let mut waited = ::std::time::Duration::ZERO;
        loop {
            $crate::downloads::telegram_monitor::flood::gate()
                .wait()
                .await;
            match $iter.next().await {
                Ok(value) => break Ok(value),
                Err(err) => match $crate::downloads::telegram_monitor::flood::flood_delay(&err) {
                    Some(delay) => {
                        if waited + delay
                            > $crate::downloads::telegram_monitor::flood::MAX_TOTAL_FLOOD_RETRY
                        {
                            break Err(err);
                        }
                        waited += delay;
                        $crate::downloads::telegram_monitor::flood::gate()
                            .defer_flood(delay)
                            .await;
                    }
                    None => break Err(err),
                },
            }
        }
    }};
}
pub(crate) use next_on_flood;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flood_wait_is_the_advertised_delay_plus_one_second() {
        assert_eq!(
            flood_wait_delay(420, Some(31)),
            Some(Duration::from_secs(32))
        );
        assert_eq!(flood_wait_delay(420, None), Some(Duration::from_secs(6)));
    }

    #[test]
    fn other_errors_have_no_flood_delay() {
        assert_eq!(flood_wait_delay(500, Some(30)), None);
        assert_eq!(flood_wait_delay(400, None), None);
    }

    #[test]
    fn absurd_flood_waits_are_clamped() {
        assert_eq!(
            flood_wait_delay(420, Some(u32::MAX)),
            Some(MAX_FLOOD_WAIT - Duration::from_secs(1))
        );
    }

    #[tokio::test]
    async fn gate_is_open_by_default() {
        let gate = FloodGate::new();
        tokio::time::timeout(Duration::from_millis(50), gate.wait())
            .await
            .expect("an open gate must not wait");
    }

    #[tokio::test]
    async fn defer_blocks_until_the_window_elapses() {
        let gate = FloodGate::new();
        gate.defer(Duration::from_millis(30)).await;

        let start = Instant::now();
        gate.wait().await;
        assert!(start.elapsed() >= Duration::from_millis(25));
    }

    #[tokio::test]
    async fn defer_never_shortens_a_running_window() {
        let gate = FloodGate::new();
        gate.defer(Duration::from_millis(60)).await;
        gate.defer(Duration::from_millis(1)).await;

        let start = Instant::now();
        gate.wait().await;
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn flood_wait_halves_the_request_budget_once_per_window() {
        let gate = FloodGate::new();
        assert_eq!(gate.file_request_limit(), MAX_CONCURRENT_FILE_REQUESTS);

        gate.defer_flood(Duration::from_millis(1)).await;
        assert_eq!(
            gate.file_request_limit(),
            MAX_CONCURRENT_FILE_REQUESTS / 2,
            "a flood wait must narrow the in-flight request budget"
        );

        // A burst of 420s from the same window must not collapse the budget.
        gate.defer_flood(Duration::from_millis(1)).await;
        assert_eq!(gate.file_request_limit(), MAX_CONCURRENT_FILE_REQUESTS / 2);
    }

    #[tokio::test]
    async fn request_budget_never_drops_below_one() {
        let gate = FloodGate::new();
        for _ in 0..MAX_CONCURRENT_FILE_REQUESTS {
            // Age the last flood so the next one counts as a new window.
            *gate
                .last_flood
                .lock()
                .unwrap_or_else(|err| err.into_inner()) =
                Some(Instant::now() - SHRINK_COOLDOWN - Duration::from_secs(1));
            gate.defer_flood(Duration::from_millis(1)).await;
        }

        assert_eq!(gate.file_request_limit(), 1);
    }

    #[tokio::test]
    async fn request_budget_recovers_one_slot_per_calm_interval() {
        let gate = FloodGate::new();
        gate.defer_flood(Duration::from_millis(1)).await;
        let shrunken = gate.file_request_limit();
        assert!(shrunken < MAX_CONCURRENT_FILE_REQUESTS);

        gate.recover(Instant::now());
        assert_eq!(
            gate.file_request_limit(),
            shrunken,
            "no slot is restored immediately after a flood"
        );

        gate.recover(Instant::now() + REQUEST_RECOVERY_INTERVAL + Duration::from_secs(1));
        assert_eq!(
            gate.file_request_limit(),
            shrunken + 1,
            "one calm interval restores exactly one request slot"
        );
    }

    #[test]
    fn connections_are_shared_across_downloads() {
        let gate = FloodGate::new();
        let mut permits: Vec<_> = std::iter::from_fn(|| gate.try_acquire_connection())
            .take(MAX_CONCURRENT_CONNECTIONS + 1)
            .collect();

        assert_eq!(
            permits.len(),
            MAX_CONCURRENT_CONNECTIONS,
            "the process-wide connection budget must cap concurrent downloads"
        );
        assert!(gate.try_acquire_connection().is_none());

        permits.clear();
        assert!(
            gate.try_acquire_connection().is_some(),
            "a finished download releases its sockets for the next one"
        );
    }

    #[tokio::test]
    async fn file_requests_are_capped_for_the_whole_process() {
        let gate = FloodGate::new();
        let mut permits = Vec::new();
        for _ in 0..MAX_CONCURRENT_FILE_REQUESTS {
            permits.push(gate.acquire_file_request().await);
        }

        let blocked =
            tokio::time::timeout(Duration::from_millis(50), gate.acquire_file_request()).await;
        assert!(
            blocked.is_err(),
            "a request beyond the global cap must wait for a slot"
        );

        permits.pop();
        let _permit = tokio::time::timeout(Duration::from_millis(50), gate.acquire_file_request())
            .await
            .expect("releasing a slot must unblock the next request");
    }
}
