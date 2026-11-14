//! Recovery of downloads orphaned by a previous process.
//!
//! A crash or a restart while a transfer is running leaves its row in
//! `downloading` forever: nothing resumes it, so the UI keeps showing a frozen
//! progress bar (and now a frozen speed/ETA). A `queued` row is just as stuck:
//! the queue lives in memory ([`super::worker::DownloadSlots`]), so the task
//! that would have claimed it died with the process. The API sweeps both once
//! at startup.
//!
//! Torrent imports left `importing` are handed back to `completed` in the same
//! sweep. It must run before the pollers are spawned and before HTTP is
//! served: once anything can reserve an import, an `importing` row may belong
//! to a live task, and resetting it would start the same import twice.

use sqlx::PgPool;

/// Reset the downloads left in `queued` or `downloading` by a previous server
/// run.
///
/// Telegram links become `available` again: their source can be re-downloaded
/// from the UI. DDL rows become `failed` instead, because there is no automatic
/// requeue for direct provider downloads — `failed` is the status that offers
/// the retry button. Both UIs refuse to relaunch a `queued` row, so leaving it
/// there would block it forever.
///
/// Also hands torrent imports interrupted by the restart back to `completed`
/// (see [`super::torrent_ledger::recover_interrupted_imports`]); the torrent
/// poller resumes them on its first pass. Callers must run this before
/// spawning the download pollers and serving HTTP.
pub async fn reset_stale_downloads(pool: &PgPool) -> Result<(), sqlx::Error> {
    match super::torrent_ledger::recover_interrupted_imports(pool).await {
        Ok(0) => {}
        Ok(n) => {
            tracing::warn!("[RECOVERY] Recovered {n} torrent import(s) interrupted by a restart")
        }
        Err(e) => tracing::warn!("[RECOVERY] Failed to recover interrupted torrent imports: {e}"),
    }

    sqlx::query(
        "UPDATE telegram_book_links \
         SET status = 'available', bytes_downloaded = 0, download_speed = 0, eta = 0, \
             error_message = NULL, updated_at = NOW() \
         WHERE status IN ('queued', 'downloading')",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE ddl_provider_downloads \
         SET status = 'failed', download_speed = 0, eta = 0, \
             error_message = 'download interrupted by a server restart', updated_at = NOW() \
         WHERE status IN ('queued', 'downloading')",
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn stale_downloads_are_recovered(pool: PgPool) {
        let source_id = Uuid::new_v4();
        sqlx::query("INSERT INTO telegram_sources (id, channel_username) VALUES ($1, $2)")
            .bind(source_id)
            .bind(format!("chan-{source_id}"))
            .execute(&pool)
            .await
            .unwrap();

        let telegram_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO telegram_book_links \
                (id, source_id, message_id, filename, status, bytes_downloaded, download_speed, eta) \
             VALUES ($1, $2, 1, 'a.cbz', 'downloading', 4096, 1024, 30)",
        )
        .bind(telegram_id)
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();

        let ddl_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ddl_provider_downloads \
                (id, provider, external_id, filename, series_name, status, download_speed, eta) \
             VALUES ($1, 'fourtoutici', 'ext-1', 'a.pdf', 'A', 'downloading', 2048, 60)",
        )
        .bind(ddl_id)
        .execute(&pool)
        .await
        .unwrap();

        // A queued row is orphaned too: its in-memory queue died with the process.
        let queued_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ddl_provider_downloads \
                (id, provider, external_id, filename, series_name, status) \
             VALUES ($1, 'fourtoutici', 'ext-2', 'b.pdf', 'B', 'queued')",
        )
        .bind(queued_id)
        .execute(&pool)
        .await
        .unwrap();

        reset_stale_downloads(&pool).await.unwrap();

        let (status, bytes, speed, eta): (String, i64, i64, i64) = sqlx::query_as(
            "SELECT status, bytes_downloaded, download_speed, eta \
             FROM telegram_book_links WHERE id = $1",
        )
        .bind(telegram_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "available");
        assert_eq!((bytes, speed, eta), (0, 0, 0));

        let (status, speed, eta, error): (String, i64, i64, Option<String>) = sqlx::query_as(
            "SELECT status, download_speed, eta, error_message \
             FROM ddl_provider_downloads WHERE id = $1",
        )
        .bind(ddl_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "failed");
        assert_eq!((speed, eta), (0, 0));
        assert!(error.is_some(), "the failure must explain the restart");

        let status: String =
            sqlx::query_scalar("SELECT status FROM ddl_provider_downloads WHERE id = $1")
                .bind(queued_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "failed", "a queued DDL download must be retryable");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn queued_telegram_link_becomes_available(pool: PgPool) {
        let source_id = Uuid::new_v4();
        sqlx::query("INSERT INTO telegram_sources (id, channel_username) VALUES ($1, $2)")
            .bind(source_id)
            .bind(format!("chan-{source_id}"))
            .execute(&pool)
            .await
            .unwrap();
        let queued_id = Uuid::new_v4();
        let imported_id = Uuid::new_v4();
        for (id, message_id, status) in [
            (queued_id, 1_i64, "queued"),
            (imported_id, 2_i64, "imported"),
        ] {
            sqlx::query(
                "INSERT INTO telegram_book_links (id, source_id, message_id, filename, status) \
                 VALUES ($1, $2, $3, 'a.cbz', $4)",
            )
            .bind(id)
            .bind(source_id)
            .bind(message_id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        }

        reset_stale_downloads(&pool).await.unwrap();

        let status_of = |id: Uuid| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, String>(
                    "SELECT status FROM telegram_book_links WHERE id = $1",
                )
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        assert_eq!(status_of(queued_id).await, "available");
        assert_eq!(
            status_of(imported_id).await,
            "imported",
            "finished rows are untouched"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn interrupted_torrent_imports_are_handed_back_at_startup(pool: PgPool) {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind(format!("lib_{library_id}"))
            .bind(format!("/libraries/{library_id}"))
            .execute(&pool)
            .await
            .unwrap();
        let importing = Uuid::new_v4();
        let downloading = Uuid::new_v4();
        for (id, status) in [(importing, "importing"), (downloading, "downloading")] {
            sqlx::query(
                "INSERT INTO torrent_downloads (id, library_id, series_name, status) \
                 VALUES ($1, $2, 'Amulet', $3)",
            )
            .bind(id)
            .bind(library_id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        }

        reset_stale_downloads(&pool).await.unwrap();

        let status_of = |id: Uuid| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, String>(
                    "SELECT status FROM torrent_downloads WHERE id = $1",
                )
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        assert_eq!(status_of(importing).await, "completed");
        assert_eq!(
            status_of(downloading).await,
            "downloading",
            "a transfer still runs in its backend"
        );
    }
}
