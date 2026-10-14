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
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let scan_mode: String = row.get("scan_mode");

        info!(
            "[SCHEDULER] Auto-scanning library {} (mode: {})",
            library_id, scan_mode
        );

        let job_id = Uuid::new_v4();
        let job_type = match scan_mode.as_str() {
            "full" => "full_rebuild",
            _ => "rebuild",
        };

        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, 'pending')",
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

        info!(
            "[SCHEDULER] Created job {} for library {}",
            job_id, library_id
        );
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
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let push_mode: String = row.get("reading_status_push_mode");

        info!(
            "[SCHEDULER] Auto-pushing reading status for library {} (mode: {})",
            library_id, push_mode
        );

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

        info!(
            "[SCHEDULER] Created reading_status_push job {} for library {}",
            job_id, library_id
        );
    }

    Ok(())
}

pub async fn check_and_schedule_download_detection(pool: &PgPool) -> Result<()> {
    // Only schedule if Prowlarr is configured. The setting JSON uses the
    // 'url' key (see apps/api/src/integrations/discovery/mod.rs), not 'base_url'.
    let prowlarr_configured: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'prowlarr' AND value->>'url' IS NOT NULL AND value->>'url' != '')"
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
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let detection_mode: String = row.get("download_detection_mode");

        info!(
            "[SCHEDULER] Auto-running download detection for library {} (mode: {})",
            library_id, detection_mode
        );

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

        info!(
            "[SCHEDULER] Created download_detection job {} for library {}",
            job_id, library_id
        );
    }

    Ok(())
}

pub async fn check_and_schedule_prowlarr_rss(pool: &PgPool) -> Result<()> {
    let prowlarr_configured: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'prowlarr' AND value->>'url' IS NOT NULL AND value->>'url' != '')"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !prowlarr_configured {
        return Ok(());
    }

    // Read configured interval (minutes); 0 means disabled, default 30
    let interval_minutes: i32 = sqlx::query_scalar(
        "SELECT COALESCE((value->>'rss_poll_interval_minutes')::int, 30) FROM app_settings WHERE key = 'prowlarr'"
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(30);

    if interval_minutes == 0 {
        return Ok(());
    }

    // One global job covers all libraries with a single RSS fetch.
    // Skip if a global job is already pending/running or finished within the configured interval.
    let already_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id IS NULL AND type = 'prowlarr_rss' AND status IN ('pending', 'running'))"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if already_active {
        return Ok(());
    }

    let recent_run: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id IS NULL AND type = 'prowlarr_rss' AND finished_at > NOW() - INTERVAL '1 minute' * $1)"
    )
    .bind(interval_minutes)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if recent_run {
        return Ok(());
    }

    let job_id = Uuid::new_v4();
    sqlx::query("INSERT INTO index_jobs (id, type, status) VALUES ($1, 'prowlarr_rss', 'pending')")
        .bind(job_id)
        .execute(pool)
        .await?;

    info!("[SCHEDULER] Created global prowlarr_rss job {}", job_id);

    Ok(())
}

pub async fn check_and_schedule_telegram_sync_incremental(pool: &PgPool) -> Result<()> {
    let authorized: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM app_settings WHERE key = 'telegram_monitor' AND value->>'session_data' IS NOT NULL)"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !authorized {
        return Ok(());
    }

    // Uses the Telegram Monitor sync interval; 0 means disabled, default 30 minutes.
    let interval_minutes: i32 = sqlx::query_scalar(
        "SELECT COALESCE((value->>'sync_interval_minutes')::int, 30) FROM app_settings WHERE key = 'telegram_monitor'"
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(30);

    if interval_minutes == 0 {
        return Ok(());
    }

    let already_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id IS NULL AND type = 'telegram_sync_incremental' AND status IN ('pending', 'running'))"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if already_active {
        return Ok(());
    }

    let recent_run: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id IS NULL AND type = 'telegram_sync_incremental' AND finished_at > NOW() - INTERVAL '1 minute' * $1)"
    )
    .bind(interval_minutes)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if recent_run {
        return Ok(());
    }

    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, type, status) VALUES ($1, 'telegram_sync_incremental', 'pending')"
    )
    .bind(job_id)
    .execute(pool)
    .await?;

    info!(
        "[SCHEDULER] Created global telegram_sync_incremental job {}",
        job_id
    );

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
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in libraries {
        let library_id: Uuid = row.get("id");
        let refresh_mode: String = row.get("metadata_refresh_mode");

        info!(
            "[SCHEDULER] Auto-refreshing metadata for library {} (mode: {})",
            library_id, refresh_mode
        );

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

        info!(
            "[SCHEDULER] Created metadata_refresh job {} for library {}",
            job_id, library_id
        );
    }

    Ok(())
}

/// Schedule the global DDL background search.
///
/// Opt-in: it only runs when at least one DDL provider is enabled **and** a
/// non-zero `search_interval_minutes` is configured in `app_settings.ddl_providers`.
pub async fn check_and_schedule_ddl_search(pool: &PgPool) -> Result<()> {
    // Settings JSON: `{"providers": {"fourtoutici": {"enabled": true, ...}}, ...}`.
    let has_enabled_provider: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM app_settings s,
                 jsonb_each(s.value->'providers') AS p(key, cfg)
            WHERE s.key = 'ddl_providers'
              AND jsonb_typeof(s.value->'providers') = 'object'
              AND COALESCE((p.cfg->>'enabled')::bool, FALSE)
        )
        "#,
    )
    .fetch_one(pool)
    .await?;

    if !has_enabled_provider {
        return Ok(());
    }

    // 0 disables the background search; default off (opt-in).
    let interval_minutes: i32 = sqlx::query_scalar(
        "SELECT COALESCE((value->>'search_interval_minutes')::int, 0) FROM app_settings WHERE key = 'ddl_providers'"
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(0);

    if interval_minutes <= 0 {
        return Ok(());
    }

    // A single global job covers every library.
    let already_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id IS NULL AND type = 'ddl_search' AND status IN ('pending', 'running'))"
    )
    .fetch_one(pool)
    .await?;

    if already_active {
        return Ok(());
    }

    let recent_run: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id IS NULL AND type = 'ddl_search' AND finished_at > NOW() - INTERVAL '1 minute' * $1)"
    )
    .bind(interval_minutes)
    .fetch_one(pool)
    .await?;

    if recent_run {
        return Ok(());
    }

    let job_id = Uuid::new_v4();
    sqlx::query("INSERT INTO index_jobs (id, type, status) VALUES ($1, 'ddl_search', 'pending')")
        .bind(job_id)
        .execute(pool)
        .await?;

    info!("[SCHEDULER] Created global ddl_search job {}", job_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn set_ddl_settings(pool: &PgPool, enabled: bool, interval_minutes: i32) {
        let value = serde_json::json!({
            "providers": { "fourtoutici": { "enabled": enabled } },
            "search_interval_minutes": interval_minutes,
        });
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('ddl_providers', $1, NOW()) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()",
        )
        .bind(value)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn ddl_search_job_count(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM index_jobs WHERE type = 'ddl_search'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn ddl_search_not_scheduled_when_no_provider_enabled(pool: sqlx::PgPool) {
        // The 0120 seed ships fourtoutici disabled.
        check_and_schedule_ddl_search(&pool).await.unwrap();
        assert_eq!(ddl_search_job_count(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn ddl_search_not_scheduled_when_interval_is_zero(pool: sqlx::PgPool) {
        set_ddl_settings(&pool, true, 0).await;
        check_and_schedule_ddl_search(&pool).await.unwrap();
        assert_eq!(ddl_search_job_count(&pool).await, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn ddl_search_scheduled_when_enabled_and_interval_set(pool: sqlx::PgPool) {
        set_ddl_settings(&pool, true, 30).await;
        check_and_schedule_ddl_search(&pool).await.unwrap();
        assert_eq!(ddl_search_job_count(&pool).await, 1);

        // The global job is created with a NULL library_id.
        let library_id: Option<Uuid> =
            sqlx::query_scalar("SELECT library_id FROM index_jobs WHERE type = 'ddl_search'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(library_id.is_none());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn ddl_search_not_duplicated_while_active_or_recent(pool: sqlx::PgPool) {
        set_ddl_settings(&pool, true, 30).await;

        // A run that finished just now must suppress a new one.
        sqlx::query(
            "INSERT INTO index_jobs (id, type, status, finished_at) \
             VALUES ($1, 'ddl_search', 'success', NOW())",
        )
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();

        check_and_schedule_ddl_search(&pool).await.unwrap();
        assert_eq!(ddl_search_job_count(&pool).await, 1);
    }
}
