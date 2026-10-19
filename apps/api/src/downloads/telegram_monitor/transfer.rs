//! Telegram-specific pieces plugged into the generic
//! [`download_ranged`](crate::downloads::ranged::download_ranged) engine:
//! connection pool, `upload.getFile` fetcher, error classification and
//! progress reporting.

use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use grammers_client::grammers_tl_types as tl;
use grammers_client::{Client, Config, InvocationError};
use grammers_session::Session;
use uuid::Uuid;

use crate::downloads::limit::DynamicLimitPermit;
use crate::downloads::ranged::{FetchError, ProgressReporter, RangeFetcher};

use super::flood;

/// The Telegram connections of one download, plus the slice of the
/// process-wide connection budget they hold.
///
/// Telegram throttles the **account**, so a third simultaneous download must
/// share the sockets the first two already opened instead of adding four more
/// logins. The permits are released when the pool is dropped, i.e. when the
/// transfer ends.
pub struct ClientPool {
    clients: Vec<Client>,
    _budget: Vec<DynamicLimitPermit<'static>>,
}

impl ClientPool {
    pub fn first(&self) -> Option<&Client> {
        self.clients.first()
    }

    pub fn len(&self) -> usize {
        self.clients.len()
    }

    #[allow(dead_code)] // symmetry with `len`, which clippy expects on a size-like type
    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Client> {
        self.clients.iter()
    }
}

/// Open up to `count` independent Telegram connections that all share the same
/// session.
///
/// Telegram caps the throughput of a single TCP connection, so spreading a
/// download over several sockets is what makes it fast; a single connection
/// tops out around 2 MB/s regardless of the local bandwidth.
///
/// Connections are opened sequentially: booting several sockets at once for the
/// same account adds a burst of `initConnection` calls that can itself trigger
/// a flood wait when another download is already running.
///
/// The sockets come out of a process-wide budget, so downloads starting later
/// get a smaller share rather than multiplying the account's connection count.
pub async fn open_clients(
    api_id: i32,
    api_hash: &str,
    session_bytes: &[u8],
    count: usize,
) -> anyhow::Result<ClientPool> {
    let requested = count.max(1);
    let gate = flood::gate();

    // Take as many sockets as the shared budget allows without blocking, so
    // two downloads that start together split it instead of queueing behind
    // each other. At least one socket is always granted: waiting for a slot is
    // preferable to running without any connection.
    let mut budget = Vec::with_capacity(requested);
    for _ in 0..requested {
        match gate.try_acquire_connection() {
            Some(permit) => budget.push(permit),
            None => break,
        }
    }
    if budget.is_empty() {
        budget.push(gate.acquire_connection().await);
    }
    if budget.len() < requested {
        tracing::info!(
            "[TG] {} of {} Telegram connections available, sharing the budget with running downloads",
            budget.len(),
            requested
        );
    }

    let mut clients = Vec::with_capacity(budget.len());
    for _ in 0..budget.len() {
        let mut waited = Duration::ZERO;
        let client = loop {
            flood::gate().wait().await;
            let session = Session::load(session_bytes)?;
            match Client::connect(Config {
                session,
                api_id,
                api_hash: api_hash.to_string(),
                params: flood::init_params(),
            })
            .await
            {
                Ok(client) => break client,
                Err(err) => match flood::authorization_flood_delay(&err) {
                    Some(delay) => {
                        if waited + delay > flood::MAX_TOTAL_FLOOD_RETRY {
                            return Err(err.into());
                        }
                        waited += delay;
                        flood::gate().defer_flood(delay).await;
                    }
                    None => return Err(err.into()),
                },
            }
        };
        clients.push(client);
    }
    Ok(ClientPool {
        clients,
        _budget: budget,
    })
}

/// Fetches byte ranges of one document through a single Telegram connection.
pub struct TelegramFetcher {
    client: Client,
    location: tl::enums::InputFileLocation,
    /// Non-zero once Telegram asked us to migrate to another datacenter.
    dc_id: AtomicI32,
}

impl TelegramFetcher {
    pub fn new(client: Client, location: tl::enums::InputFileLocation) -> Self {
        Self {
            client,
            location,
            dc_id: AtomicI32::new(0),
        }
    }
}

/// `upload.getFile` rejects a `limit` that is not a multiple of 4 KiB with
/// `400 LIMIT_INVALID`. The final block of a file is a short remainder, so the
/// request has to be rounded up; the server then returns only the bytes left.
const REQUEST_ALIGNMENT: i32 = 4 * 1024;

fn aligned_limit(limit: i32) -> i32 {
    // `i32::div_ceil` is still unstable, and `limit` is always positive here.
    (limit + REQUEST_ALIGNMENT - 1) / REQUEST_ALIGNMENT * REQUEST_ALIGNMENT
}

#[async_trait]
impl RangeFetcher for TelegramFetcher {
    async fn fetch(&self, offset: i64, limit: i32) -> Result<Vec<u8>, FetchError> {
        // `precise = true` makes the server honour `offset` exactly, which is
        // what lets several workers fill the file out of order.
        let request = tl::functions::upload::GetFile {
            precise: true,
            cdn_supported: false,
            location: self.location.clone(),
            offset,
            limit: aligned_limit(limit),
        };

        // Every connection of every running download shares the same gate and
        // the same request budget: flood waits are per account, so one blocked
        // worker must pause the others instead of letting them extend it.
        let gate = flood::gate();
        gate.wait().await;
        let permit = gate.acquire_file_request().await;
        // A flood wait may have been reported by another worker while this one
        // was queued for a request slot.
        gate.wait().await;

        let dc_id = self.dc_id.load(Ordering::Relaxed);
        let result = if dc_id == 0 {
            self.client.invoke(&request).await
        } else {
            self.client.invoke_in_dc(&request, dc_id).await
        };

        match result {
            Ok(tl::enums::upload::File::File(file)) => {
                drop(permit);
                // A success is the signal that Telegram has calmed down; the
                // gate may restore one request slot if enough time has passed.
                gate.record_success();
                Ok(file.bytes)
            }
            Ok(tl::enums::upload::File::CdnRedirect(_)) => {
                drop(permit);
                Err(FetchError::fatal(
                    "Telegram returned a CDN redirect although cdn_supported = false",
                ))
            }
            Err(err) => {
                let (error, migrate_to) = classify_invocation_error(err);
                if let Some(delay) = error.retry_after {
                    // Defer (and narrow the request budget) before releasing the
                    // slot so a queued worker sees the gate as soon as it starts.
                    gate.defer_flood(delay).await;
                }
                drop(permit);
                if let Some(dc) = migrate_to {
                    self.dc_id.store(dc, Ordering::Relaxed);
                }
                Err(error)
            }
        }
    }
}

/// Map an [`upload.getFile`] failure to a retry decision, plus the datacenter to
/// migrate to when Telegram requests it.
pub fn classify_invocation_error(err: InvocationError) -> (FetchError, Option<i32>) {
    match err {
        InvocationError::Rpc(rpc) => classify_rpc(rpc.code, &rpc.name, rpc.value),
        InvocationError::Dropped => (FetchError::transient("request dropped"), None),
        InvocationError::Read(read) => (
            FetchError::transient(format!("connection error: {read}")),
            None,
        ),
    }
}

/// Decide how to react to an RPC error.
///
/// * `420` (`FLOOD_WAIT` / `FLOOD_PREMIUM_WAIT`) → retry after the advertised
///   delay.
/// * `303 FILE_MIGRATE` → retry, on the datacenter Telegram points to.
/// * `5xx` → transient.
/// * anything else (`400 FILE_REFERENCE_EXPIRED`, `401`, `403`, …) → fatal.
fn classify_rpc(code: i32, name: &str, value: Option<u32>) -> (FetchError, Option<i32>) {
    if code == 303 && name.starts_with("FILE_MIGRATE") {
        let message = format!("rpc error {code}: {name}");
        return (FetchError::transient(message), value.map(|dc| dc as i32));
    }
    if let Some(delay) = flood::flood_wait_delay(code, value) {
        // `open_clients` disables grammers' per-connection auto-sleep, so every
        // 420 reaches this point and the process-wide gate pauses all workers.
        return (
            FetchError::retry_in(format!("rpc error {code}: {name}"), delay),
            None,
        );
    }
    if code >= 500 {
        return (
            FetchError::transient(format!("rpc error {code}: {name}")),
            None,
        );
    }
    (FetchError::fatal(format!("rpc error {code}: {name}")), None)
}

/// Persists download progress to `telegram_book_links.bytes_downloaded`.
pub struct LinkProgress {
    pool: sqlx::PgPool,
    link_id: Uuid,
}

impl LinkProgress {
    pub fn new(pool: sqlx::PgPool, link_id: Uuid) -> Self {
        Self { pool, link_id }
    }
}

#[async_trait]
impl ProgressReporter for LinkProgress {
    async fn report(&self, bytes_written: i64) {
        if let Err(err) = sqlx::query(
            "UPDATE telegram_book_links SET bytes_downloaded = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(bytes_written)
        .bind(self.link_id)
        .execute(&self.pool)
        .await
        {
            tracing::debug!("[TG] progress update failed for {}: {err}", self.link_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::downloads::ranged::FetchErrorKind;

    #[test]
    fn request_limits_are_rounded_up_to_the_telegram_block_size() {
        // A short final block must never be sent as-is: Telegram answers a
        // non-4 KiB-aligned limit with `400 LIMIT_INVALID`.
        assert_eq!(aligned_limit(1), 4096);
        assert_eq!(aligned_limit(4096), 4096);
        assert_eq!(aligned_limit(4097), 8192);
        assert_eq!(aligned_limit(512 * 1024), 512 * 1024);
    }

    #[test]
    fn flood_wait_is_retried_after_the_advertised_delay() {
        let (error, migrate_to) = classify_rpc(420, "FLOOD_WAIT", Some(31));
        assert_eq!(error.kind, FetchErrorKind::Transient);
        assert_eq!(error.retry_after, Some(Duration::from_secs(32)));
        assert_eq!(migrate_to, None);
    }

    #[test]
    fn flood_wait_without_value_uses_a_short_default() {
        let (error, _) = classify_rpc(420, "FLOOD_PREMIUM_WAIT", None);
        assert_eq!(error.kind, FetchErrorKind::Transient);
        assert_eq!(error.retry_after, Some(Duration::from_secs(6)));
    }

    #[test]
    fn server_errors_are_transient_without_a_retry_delay() {
        for code in [500, 502, 503] {
            let (error, _) = classify_rpc(code, "INTERDC_CALL_ERROR", Some(2));
            assert_eq!(error.kind, FetchErrorKind::Transient, "code {code}");
            assert_eq!(error.retry_after, None, "code {code}");
        }
    }

    #[test]
    fn file_migrate_targets_the_new_datacenter() {
        let (error, migrate_to) = classify_rpc(303, "FILE_MIGRATE_2", Some(2));
        assert_eq!(error.kind, FetchErrorKind::Transient);
        assert_eq!(migrate_to, Some(2));
    }

    #[test]
    fn client_errors_are_fatal() {
        for (code, name) in [
            (400, "FILE_REFERENCE_EXPIRED"),
            (401, "AUTH_KEY_INVALID"),
            (403, "CHANNEL_PRIVATE"),
        ] {
            let (error, _) = classify_rpc(code, name, None);
            assert_eq!(error.kind, FetchErrorKind::Fatal, "{code} {name}");
        }
    }
}
