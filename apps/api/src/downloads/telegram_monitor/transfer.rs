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

use crate::downloads::ranged::{FetchError, ProgressReporter, RangeFetcher};

/// Open `count` independent Telegram connections that all share the same
/// session.
///
/// Telegram caps the throughput of a single TCP connection, so spreading a
/// download over several sockets is what makes it fast; a single connection
/// tops out around 2 MB/s regardless of the local bandwidth.
pub async fn open_clients(
    api_id: i32,
    api_hash: &str,
    session_bytes: &[u8],
    count: usize,
) -> anyhow::Result<Vec<Client>> {
    let futures = (0..count.max(1)).map(|_| {
        let session_bytes = session_bytes.to_vec();
        let api_hash = api_hash.to_string();
        async move {
            let session = Session::load(&session_bytes)?;
            let client = Client::connect(Config {
                session,
                api_id,
                api_hash,
                params: Default::default(),
            })
            .await?;
            anyhow::Ok(client)
        }
    });
    let clients = futures::future::try_join_all(futures).await?;
    Ok(clients)
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
            limit,
        };

        let dc_id = self.dc_id.load(Ordering::Relaxed);
        let result = if dc_id == 0 {
            self.client.invoke(&request).await
        } else {
            self.client.invoke_in_dc(&request, dc_id).await
        };

        match result {
            Ok(tl::enums::upload::File::File(file)) => Ok(file.bytes),
            Ok(tl::enums::upload::File::CdnRedirect(_)) => Err(FetchError::fatal(
                "Telegram returned a CDN redirect although cdn_supported = false",
            )),
            Err(err) => {
                let (error, migrate_to) = classify_invocation_error(err);
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
    if code == 420 {
        // Grammers already sleeps out short flood waits; a 420 reaching us means
        // the delay is long, so honour it explicitly.
        let seconds = u64::from(value.unwrap_or(5)) + 1;
        return (
            FetchError::retry_in(
                format!("rpc error {code}: {name}"),
                Duration::from_secs(seconds),
            ),
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
    use super::*;
    use crate::downloads::ranged::FetchErrorKind;

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
