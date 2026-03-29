use anyhow::Result;
use sqlx::{PgPool, Row};
use stripstream_core::schedule::mode_to_interval_minutes;
use tracing::info;
use uuid::Uuid;

pub async fn check_and_schedule_auto_scans(pool: &PgPool) -> Result<()> {
    let libraries = sqlx::query(
        r#"
        SELECT id, scan_mode, last_scan_at
        FROM libraries
        WHERE monitor_enabled = TRUE
          AND (
            next_scan_at IS NULL
            OR next_scan_at <= NOW()
          )
          AND NOT EXISTS (
            SELECT 1 FROM index_jobs
            WHERE library_id = libraries.id
              AND status IN ('pending', 'running')
          )
        "#
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let scan_mode: String = row.get("scan_mode");

        info!("[SCHEDULER] Auto-scanning library {} (mode: {})", library_id, scan_mode);

        let job_id = Uuid::new_v4();
        let job_type = match scan_mode.as_str() {
            "full" => "full_rebuild",
            _ => "rebuild",
        };

        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, 'pending')"
        )
        .bind(job_id)
        .bind(library_id)
        .bind(job_type)
        .execute(pool)
        .await?;

        // Update next_scan_at
        let interval_minutes = mode_to_interval_minutes(&scan_mode);

        sqlx::query(
            "UPDATE libraries SET last_scan_at = NOW(), next_scan_at = NOW() + INTERVAL '1 minute' * $2 WHERE id = $1"
        )
        .bind(library_id)
        .bind(interval_minutes)
        .execute(pool)
        .await?;

        info!("[SCHEDULER] Created job {} for library {}", job_id, library_id);
    }

    Ok(())
}

pub async fn check_and_schedule_reading_status_push(pool: &PgPool) -> Result<()> {
    let libraries = sqlx::query(
        r#"
        SELECT id, reading_status_push_mode
        FROM libraries
        WHERE reading_status_push_mode != 'manual'
          AND reading_status_provider IS NOT NULL
          AND (
            next_reading_status_push_at IS NULL
            OR next_reading_status_push_at <= NOW()
          )
          AND NOT EXISTS (
            SELECT 1 FROM index_jobs
            WHERE library_id = libraries.id
              AND type = 'reading_status_push'
              AND status IN ('pending', 'running')
          )
          AND EXISTS (
            SELECT 1 FROM anilist_series_links
            WHERE library_id = libraries.id
          )
        "#
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let push_mode: String = row.get("reading_status_push_mode");

        info!("[SCHEDULER] Auto-pushing reading status for library {} (mode: {})", library_id, push_mode);

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'reading_status_push', 'pending')"
        )
        .bind(job_id)
        .bind(library_id)
        .execute(pool)
        .await?;

        let interval_minutes = mode_to_interval_minutes(&push_mode);

        sqlx::query(
            "UPDATE libraries SET last_reading_status_push_at = NOW(), next_reading_status_push_at = NOW() + INTERVAL '1 minute' * $2 WHERE id = $1"
        )
        .bind(library_id)
        .bind(interval_minutes)
        .execute(pool)
        .await?;

        info!("[SCHEDULER] Created reading_status_push job {} for library {}", job_id, library_id);
    }

    Ok(())
}

pub async fn check_and_schedule_download_detection(pool: &PgPool) -> Result<()> {
    // Only schedule if Prowlarr is configured
    let prowlarr_configured: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM settings WHERE key = 'prowlarr' AND value->>'base_url' IS NOT NULL AND value->>'base_url' != '')"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !prowlarr_configured {
        return Ok(());
    }

    let libraries = sqlx::query(
        r#"
        SELECT id, download_detection_mode
        FROM libraries
        WHERE download_detection_mode != 'manual'
          AND (
            next_download_detection_at IS NULL
            OR next_download_detection_at <= NOW()
          )
          AND NOT EXISTS (
            SELECT 1 FROM index_jobs
            WHERE library_id = libraries.id
              AND type = 'download_detection'
              AND status IN ('pending', 'running')
          )
        "#
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let detection_mode: String = row.get("download_detection_mode");

        info!("[SCHEDULER] Auto-running download detection for library {} (mode: {})", library_id, detection_mode);

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'download_detection', 'pending')"
        )
        .bind(job_id)
        .bind(library_id)
        .execute(pool)
        .await?;

        let interval_minutes = mode_to_interval_minutes(&detection_mode);

        sqlx::query(
            "UPDATE libraries SET last_download_detection_at = NOW(), next_download_detection_at = NOW() + INTERVAL '1 minute' * $2 WHERE id = $1"
        )
        .bind(library_id)
        .bind(interval_minutes)
        .execute(pool)
        .await?;

        info!("[SCHEDULER] Created download_detection job {} for library {}", job_id, library_id);
    }

    Ok(())
}

pub async fn check_and_schedule_metadata_refreshes(pool: &PgPool) -> Result<()> {
    let libraries = sqlx::query(
        r#"
        SELECT id, metadata_refresh_mode
        FROM libraries
        WHERE metadata_refresh_mode != 'manual'
          AND (
            next_metadata_refresh_at IS NULL
            OR next_metadata_refresh_at <= NOW()
          )
          AND NOT EXISTS (
            SELECT 1 FROM index_jobs
            WHERE library_id = libraries.id
              AND type = 'metadata_refresh'
              AND status IN ('pending', 'running')
          )
          AND EXISTS (
            SELECT 1 FROM external_metadata_links
            WHERE library_id = libraries.id
              AND status = 'approved'
          )
        "#
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let refresh_mode: String = row.get("metadata_refresh_mode");

        info!("[SCHEDULER] Auto-refreshing metadata for library {} (mode: {})", library_id, refresh_mode);

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'metadata_refresh', 'pending')"
        )
        .bind(job_id)
        .bind(library_id)
        .execute(pool)
        .await?;

        let interval_minutes = mode_to_interval_minutes(&refresh_mode);

        sqlx::query(
            "UPDATE libraries SET last_metadata_refresh_at = NOW(), next_metadata_refresh_at = NOW() + INTERVAL '1 minute' * $2 WHERE id = $1"
        )
        .bind(library_id)
        .bind(interval_minutes)
        .execute(pool)
        .await?;

        info!("[SCHEDULER] Created metadata_refresh job {} for library {}", job_id, library_id);
    }

    Ok(())
}
