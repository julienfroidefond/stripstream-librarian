//! Recovery of downloads orphaned by a previous process.
//!
//! A crash or a restart while a transfer is running leaves its row in
//! `downloading` forever: nothing resumes it, so the UI keeps showing a frozen
//! progress bar (and now a frozen speed/ETA). The API sweeps those rows once at
//! startup.

use sqlx::PgPool;

/// Reset the downloads left in `downloading` by a previous server run.
///
/// Telegram links become `available` again: their source can be re-downloaded
/// from the UI. DDL rows become `failed` instead, because there is no automatic
/// requeue for direct provider downloads — `failed` is the status that offers
/// the retry button.
pub async fn reset_stale_downloads(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE telegram_book_links \
         SET status = 'available', bytes_downloaded = 0, download_speed = 0, eta = 0, \
             error_message = NULL, updated_at = NOW() \
         WHERE status = 'downloading'",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE ddl_provider_downloads \
         SET status = 'failed', download_speed = 0, eta = 0, \
             error_message = 'download interrupted by a server restart', updated_at = NOW() \
         WHERE status = 'downloading'",
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

        // A queued row must be left alone.
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
        assert_eq!(status, "queued", "a queued download is not orphaned");
    }
}
