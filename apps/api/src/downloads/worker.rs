//! Shared lifecycle for background "download then import" tasks.
//!
//! Telegram media and DDL-provider HTTP downloads follow the same shape: a row
//! is queued, the transfer runs under a global concurrency limit, it can be
//! cancelled from the UI, and a failure is recorded back on the row. This module
//! owns that lifecycle so both flows stay in lock-step; each one only supplies
//! its SQL through [`DownloadQueue`] and its transfer through a closure.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::AbortHandle;
use uuid::Uuid;

/// Boxed future produced by a download body.
pub type DownloadFuture = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

/// The SQL that differs between download queues (Telegram book links vs DDL
/// provider downloads).
#[async_trait::async_trait]
pub trait DownloadQueue: Send + Sync + 'static {
    /// Claims the row once a permit is free. Returning `false` means the
    /// download was dismissed (or deleted) while waiting, so the transfer must
    /// be skipped.
    async fn claim(&self, pool: &PgPool, id: Uuid) -> bool;

    /// Records a failed transfer on the row.
    async fn mark_failed(&self, pool: &PgPool, id: Uuid, error: &str);
}

/// Table-driven [`DownloadQueue`] shared by the download tables.
///
/// Every "queued → downloading → failed" table has the same shape (`id`,
/// `status`, `error_message`, `updated_at`), so the claim/fail SQL only differs
/// by the table name. `table` comes from a compile-time constant in this crate
/// — never from user input — so interpolating it is safe and keeps a single SQL
/// definition instead of one near-identical copy per download flow.
#[derive(Debug, Clone, Copy)]
pub struct SqlDownloadQueue {
    table: &'static str,
}

impl SqlDownloadQueue {
    pub const fn new(table: &'static str) -> Self {
        Self { table }
    }
}

/// Telegram book downloads (`telegram_book_links`).
pub const TELEGRAM_DOWNLOAD_QUEUE: SqlDownloadQueue = SqlDownloadQueue::new("telegram_book_links");

/// Direct provider downloads (`ddl_provider_downloads`).
pub const DDL_DOWNLOAD_QUEUE: SqlDownloadQueue = SqlDownloadQueue::new("ddl_provider_downloads");

#[async_trait::async_trait]
impl DownloadQueue for SqlDownloadQueue {
    async fn claim(&self, pool: &PgPool, id: Uuid) -> bool {
        let sql = format!(
            "UPDATE {} SET status = 'downloading', error_message = NULL, updated_at = NOW() \
             WHERE id = $1 AND status = 'queued'",
            self.table
        );
        sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
            .unwrap_or(false)
    }

    async fn mark_failed(&self, pool: &PgPool, id: Uuid, error: &str) {
        // Never resurrect a download the user dismissed while it was failing.
        let sql = format!(
            "UPDATE {} SET status = 'failed', error_message = $1, updated_at = NOW() \
             WHERE id = $2 AND status <> 'dismissed'",
            self.table
        );
        let _ = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(error)
            .bind(id)
            .execute(pool)
            .await;
    }
}

/// A concurrency limit plus the abort handles of its running downloads.
#[derive(Clone)]
pub struct DownloadSlots {
    limit: Arc<Semaphore>,
    /// `id -> (token, handle)`. The token identifies the spawn that owns the
    /// handle so a late-finishing task cannot evict a newer transfer's handle.
    abort_handles: Arc<Mutex<HashMap<Uuid, (Uuid, AbortHandle)>>>,
}

impl DownloadSlots {
    pub fn new(
        limit: Arc<Semaphore>,
        abort_handles: Arc<Mutex<HashMap<Uuid, (Uuid, AbortHandle)>>>,
    ) -> Self {
        Self {
            limit,
            abort_handles,
        }
    }

    /// Cancels a queued/running download if it is still registered.
    pub async fn cancel(&self, id: Uuid) {
        if let Some((_, handle)) = self.abort_handles.lock().await.remove(&id) {
            handle.abort();
        }
    }

    /// Spawns `run` once a permit is free and `queue.claim` confirmed the row is
    /// still pending. A transfer error is reported through `queue.mark_failed`.
    pub async fn spawn<F>(&self, pool: PgPool, id: Uuid, queue: Arc<dyn DownloadQueue>, run: F)
    where
        F: FnOnce(PgPool) -> DownloadFuture + Send + 'static,
    {
        let limit = self.limit.clone();
        let abort_handles = self.abort_handles.clone();
        let token = Uuid::new_v4();

        // Register the handle while holding the map lock: a task that finishes
        // immediately (and calls `release`) cannot run before the handle exists,
        // which would otherwise leak a completed download's abort handle.
        let mut guard = self.abort_handles.lock().await;
        let task = tokio::spawn(async move {
            let _permit = limit.acquire_owned().await;

            if !queue.claim(&pool, id).await {
                release(&abort_handles, id, token).await;
                return;
            }

            if let Err(err) = run(pool.clone()).await {
                tracing::warn!("[download] {id} failed: {err:#}");
                queue.mark_failed(&pool, id, &err.to_string()).await;
            }

            release(&abort_handles, id, token).await;
        });

        guard.insert(id, (token, task.abort_handle()));
    }
}

/// Removes the abort handle for `id` only when it still belongs to `token`, so a
/// task that finishes late cannot evict a newer transfer's handle.
async fn release(handles: &Mutex<HashMap<Uuid, (Uuid, AbortHandle)>>, id: Uuid, token: Uuid) {
    let mut guard = handles.lock().await;
    if guard.get(&id).map(|(owner, _)| *owner) == Some(token) {
        guard.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;
    use std::time::Duration;

    use tokio::sync::mpsc;

    use super::*;

    /// A queue that records calls and signals every terminal step on a channel,
    /// so tests can await the spawned task deterministically.
    struct RecordingQueue {
        allowed: bool,
        claimed: AtomicUsize,
        failed: StdMutex<Vec<String>>,
        signal: mpsc::UnboundedSender<()>,
    }

    #[async_trait::async_trait]
    impl DownloadQueue for RecordingQueue {
        async fn claim(&self, _pool: &PgPool, _id: Uuid) -> bool {
            self.claimed.fetch_add(1, Ordering::SeqCst);
            if !self.allowed {
                let _ = self.signal.send(());
            }
            self.allowed
        }

        async fn mark_failed(&self, _pool: &PgPool, _id: Uuid, error: &str) {
            self.failed.lock().unwrap().push(error.to_string());
            let _ = self.signal.send(());
        }
    }

    fn slots() -> DownloadSlots {
        DownloadSlots::new(
            Arc::new(Semaphore::new(1)),
            Arc::new(Mutex::new(HashMap::new())),
        )
    }

    async fn recv(signal: &mut mpsc::UnboundedReceiver<()>) {
        tokio::time::timeout(Duration::from_secs(5), signal.recv())
            .await
            .expect("spawned task did not signal")
            .expect("signal channel closed");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn spawn_skips_the_transfer_when_claim_is_refused(pool: PgPool) {
        let (signal, mut rx) = mpsc::unbounded_channel();
        let queue = Arc::new(RecordingQueue {
            allowed: false,
            claimed: AtomicUsize::new(0),
            failed: StdMutex::new(Vec::new()),
            signal,
        });
        let ran = Arc::new(AtomicBool::new(false));
        let ran_clone = ran.clone();

        slots()
            .spawn(pool, Uuid::new_v4(), queue.clone(), move |_pool| {
                let ran = ran_clone.clone();
                Box::pin(async move {
                    ran.store(true, Ordering::SeqCst);
                    Ok::<(), anyhow::Error>(())
                })
            })
            .await;

        recv(&mut rx).await;

        assert_eq!(queue.claimed.load(Ordering::SeqCst), 1);
        assert!(!ran.load(Ordering::SeqCst));
        assert!(queue.failed.lock().unwrap().is_empty());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn spawn_reports_transfer_failures_on_the_queue(pool: PgPool) {
        let (signal, mut rx) = mpsc::unbounded_channel();
        let queue = Arc::new(RecordingQueue {
            allowed: true,
            claimed: AtomicUsize::new(0),
            failed: StdMutex::new(Vec::new()),
            signal,
        });

        slots()
            .spawn(pool, Uuid::new_v4(), queue.clone(), move |_pool| {
                Box::pin(async move { Err::<(), anyhow::Error>(anyhow::anyhow!("boom")) })
            })
            .await;

        recv(&mut rx).await;

        assert_eq!(queue.claimed.load(Ordering::SeqCst), 1);
        let failed = queue.failed.lock().unwrap();
        assert_eq!(failed.len(), 1);
        assert!(failed[0].contains("boom"));
    }

    #[tokio::test]
    async fn cancel_is_a_noop_for_unknown_ids() {
        // Just exercises the lock/remove path without panicking.
        slots().cancel(Uuid::new_v4()).await;
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn sql_queue_generates_valid_sql_for_every_table(pool: PgPool) {
        // Runs the generated SQL against both download tables: a typo in a table
        // or column name would surface here as a real query error.
        let telegram_id = insert_telegram_link(&pool, "queued").await;
        let ddl_id = insert_ddl_download(&pool, "queued").await;

        for (queue, id) in [
            (TELEGRAM_DOWNLOAD_QUEUE, telegram_id),
            (DDL_DOWNLOAD_QUEUE, ddl_id),
        ] {
            assert!(queue.claim(&pool, id).await);
            // A row already claimed is no longer 'queued'.
            assert!(!queue.claim(&pool, id).await);
            queue.mark_failed(&pool, id, "boom").await;
        }
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn sql_queue_claim_clears_a_stale_error(pool: PgPool) {
        let id = insert_telegram_link(&pool, "queued").await;
        sqlx::query("UPDATE telegram_book_links SET error_message = 'old' WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        assert!(TELEGRAM_DOWNLOAD_QUEUE.claim(&pool, id).await);

        let (status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status, error_message FROM telegram_book_links WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "downloading");
        assert_eq!(error, None, "a requeued row must not show a stale error");

        TELEGRAM_DOWNLOAD_QUEUE.mark_failed(&pool, id, "boom").await;
        let (status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status, error_message FROM telegram_book_links WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(error.as_deref(), Some("boom"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn sql_queue_never_resurrects_a_dismissed_row(pool: PgPool) {
        let id = insert_ddl_download(&pool, "dismissed").await;

        assert!(!DDL_DOWNLOAD_QUEUE.claim(&pool, id).await);
        DDL_DOWNLOAD_QUEUE.mark_failed(&pool, id, "boom").await;

        let status: String =
            sqlx::query_scalar("SELECT status FROM ddl_provider_downloads WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "dismissed");
    }

    /// Insert a `telegram_book_links` row with the given status.
    async fn insert_telegram_link(pool: &PgPool, status: &str) -> Uuid {
        let source_id = Uuid::new_v4();
        sqlx::query("INSERT INTO telegram_sources (id, channel_username) VALUES ($1, $2)")
            .bind(source_id)
            .bind(format!("chan-{source_id}"))
            .execute(pool)
            .await
            .unwrap();

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO telegram_book_links (id, source_id, message_id, filename, status) \
             VALUES ($1, $2, 1, 'a.cbz', $3)",
        )
        .bind(id)
        .bind(source_id)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    /// Insert a `ddl_provider_downloads` row with the given status.
    async fn insert_ddl_download(pool: &PgPool, status: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ddl_provider_downloads \
                (id, provider, external_id, filename, series_name, status) \
             VALUES ($1, 'fourtoutici', 'ext-1', 'a.pdf', 'A', $2)",
        )
        .bind(id)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
        id
    }
}
