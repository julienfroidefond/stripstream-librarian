//! Integration tests for the `diagnostics` module.
//!
//! The handler is called directly with the shared `test_state` helper, backed by
//! an ephemeral Postgres provisioned by `#[sqlx::test]`.

use std::time::Duration;

use axum::extract::State;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use super::*;
use crate::state::test_state;

// ─── format_bytes ────────────────────────────────────────────────────────────

#[test]
fn format_bytes_keeps_small_values_in_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1), "1 B");
    assert_eq!(format_bytes(1023), "1023 B");
}

#[test]
fn format_bytes_scales_to_the_largest_fitting_unit() {
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MB");
    assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
    assert_eq!(format_bytes(3 * 1024_u64.pow(4)), "3.0 TB");
}

#[test]
fn format_bytes_does_not_overflow_on_huge_values() {
    assert_eq!(format_bytes(u64::MAX), "16777216.0 TB");
}

// ─── get_diagnostics ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn diagnostics_aggregates_libraries_jobs_and_files(pool: PgPool) {
    let state = test_state(pool.clone());
    let lib_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO libraries (id, name, root_path, enabled, last_scan_at) \
         VALUES ($1, 'Gon', '/libraries/gon', true, '2026-01-02T03:04:05Z')",
    )
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    // One fully analysed book with a thumbnail and a 2 KB file…
    let analysed_book = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, kind, title, format, volume, page_count, thumbnail_path) \
         VALUES ($1, $2, 'bd', 'Gon T01', 'cbz', 1, 12, '/data/thumbnails/gon-1.webp')",
    )
    .bind(analysed_book)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) \
         VALUES ($1, $2, 'cbz', '/libraries/gon/gon-1.cbz', 2048, NOW(), 'fp-1', 'ok')",
    )
    .bind(Uuid::new_v4())
    .bind(analysed_book)
    .execute(&pool)
    .await
    .unwrap();

    // …and one discovered but not analysed yet.
    let pending_book = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, kind, title, format, volume) \
         VALUES ($1, $2, 'bd', 'Gon T02', 'cbr', 2)",
    )
    .bind(pending_book)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) \
         VALUES ($1, $2, 'cbr', '/libraries/gon/gon-2.cbr', 1024, NOW(), 'fp-2', 'ok')",
    )
    .bind(Uuid::new_v4())
    .bind(pending_book)
    .execute(&pool)
    .await
    .unwrap();

    let failed_job = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at, finished_at, error_opt) \
         VALUES ($1, $2, 'scan', 'failed', NOW() - INTERVAL '2 hours', NOW() - INTERVAL '1 hour', 'boom')",
    )
    .bind(failed_job)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, started_at) \
         VALUES ($1, $2, 'scan', 'running', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let Json(report) = get_diagnostics(State(state)).await.unwrap();

    assert_eq!(report.version, env!("CARGO_PKG_VERSION"));
    assert!(report.uptime_seconds < 60);

    // Database
    assert!(report.database.reachable);
    assert!(report.database.migrations_applied > 0);
    assert!(report.database.latest_migration.is_some());
    assert!(report.database.size_bytes > 0);
    assert_ne!(report.database.size_pretty, "0 B");

    // Cache: the in-memory page cache is exposed, including hit counters.
    assert!(report.cache.memory_max_size_mb > 0.0);
    assert_eq!(report.cache.hits, 0);
    assert_eq!(report.cache.misses, 0);

    // Thumbnails: unresolved books are counted even when the directory walk is empty.
    assert_eq!(report.thumbnails.missing, 1);
    assert_eq!(report.thumbnails.pending_analysis, 1);
    assert!(!report.thumbnails.directory.is_empty());

    // Jobs
    assert_eq!(report.jobs.total, 3);
    assert_eq!(report.jobs.pending, 1);
    assert_eq!(report.jobs.running, 1);
    assert_eq!(report.jobs.failed_last_24h, 1);
    assert_eq!(report.jobs.recent_failures.len(), 1);
    let failure = &report.jobs.recent_failures[0];
    assert_eq!(failure.id, failed_job);
    assert_eq!(failure.job_type, "scan");
    assert_eq!(failure.library_name.as_deref(), Some("Gon"));
    assert_eq!(failure.error.as_deref(), Some("boom"));

    // Libraries
    assert_eq!(report.libraries.len(), 1);
    let library = &report.libraries[0];
    assert_eq!(library.id, lib_id);
    assert_eq!(library.name, "Gon");
    assert!(library.enabled);
    assert_eq!(library.book_count, 2);
    assert_eq!(library.pending_analysis, 1);
    assert_eq!(library.missing_thumbnails, 1);
    assert_eq!(library.size_bytes, 3072);
    assert_eq!(library.size_pretty, "3.0 KB");
    assert_eq!(
        library.last_scan_at.map(|t| t.to_rfc3339()),
        Some("2026-01-02T03:04:05+00:00".to_string())
    );

    // Job breakdown: the three jobs are all `scan`, none of them stuck.
    assert_eq!(report.jobs.stale_pending, 0);
    assert_eq!(report.jobs.by_type.len(), 1);
    assert_eq!(report.jobs.by_type[0].job_type, "scan");
    assert_eq!(report.jobs.by_type[0].pending, 1);
    assert_eq!(report.jobs.by_type[0].running, 1);
    assert_eq!(report.jobs.by_type[0].failed_last_24h, 1);

    // No heartbeat rows and nothing wrong with the data set.
    assert!(report.services.is_empty());
    assert_eq!(report.integrity.books_without_files, 0);
    assert_eq!(report.integrity.books_zero_pages, 0);

    // Runtime: the render slot counter is wired to the semaphore.
    assert_eq!(report.runtime.render_limit, 1);
    assert_eq!(report.runtime.renders_in_flight, 0);

    // Disks: one entry per volume, always present even when unavailable.
    assert_eq!(report.disks.len(), 3);
    let ids: Vec<_> = report.disks.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["libraries", "thumbnails", "image_cache"]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn diagnostics_reports_online_and_offline_services(pool: PgPool) {
    let state = test_state(pool.clone());

    sqlx::query(
        "INSERT INTO service_heartbeats (service, version, started_at, last_seen_at) VALUES \
         ('api', '1.2.3', NOW() - INTERVAL '2 hours', NOW()), \
         ('indexer', '1.0.0', NOW() - INTERVAL '3 hours', NOW() - INTERVAL '10 minutes')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let Json(report) = get_diagnostics(State(state)).await.unwrap();

    assert_eq!(report.services.len(), 2);
    assert_eq!(report.services[0].service, "api");
    assert!(report.services[0].online);
    assert_eq!(report.services[0].version, "1.2.3");
    assert!(report.services[0].uptime_seconds >= 2 * 3600 - 5);

    assert_eq!(report.services[1].service, "indexer");
    assert!(!report.services[1].online);
    assert!(report.services[1].seconds_since_seen >= 10 * 60);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn diagnostics_flags_stuck_pending_jobs(pool: PgPool) {
    let state = test_state(pool.clone());
    let lib_id = Uuid::new_v4();

    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Gon', '/libraries/gon')")
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    // Older than the 30-minute stale threshold.
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, created_at) \
         VALUES ($1, $2, 'scan', 'pending', NOW() - INTERVAL '2 hours')",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    // Recent, healthy pending job.
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, created_at) \
         VALUES ($1, $2, 'scan', 'pending', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let Json(report) = get_diagnostics(State(state)).await.unwrap();

    assert_eq!(report.jobs.pending, 2);
    assert_eq!(report.jobs.stale_pending, 1);
    assert!(report.jobs.oldest_pending_seconds.unwrap_or(0) >= 2 * 3600 - 5);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn diagnostics_counts_integrity_problems(pool: PgPool) {
    let state = test_state(pool.clone());
    let lib_id = Uuid::new_v4();

    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Gon', '/libraries/gon')")
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    // A series with no metadata link, holding two books of the same volume…
    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Gon')")
        .bind(series_id)
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, kind, title, format, volume, volume_type) \
         VALUES ($1, $2, $3, 'bd', 'Gon T01', 'cbz', 1, 'regular'), \
                ($4, $2, $3, 'bd', 'Gon T01 bis', 'cbr', 1, 'regular')",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .bind(series_id)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .unwrap();

    // …and an analysed book reporting zero pages.
    sqlx::query(
        "INSERT INTO books (id, library_id, kind, title, format, page_count) \
         VALUES ($1, $2, 'bd', 'Broken', 'cbz', 0)",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let Json(report) = get_diagnostics(State(state)).await.unwrap();

    assert_eq!(report.integrity.series_without_metadata, 1);
    assert_eq!(report.integrity.duplicate_volume_groups, 1);
    assert_eq!(report.integrity.books_zero_pages, 1);
    // Every book inserted above lacks a `book_files` row.
    assert_eq!(report.integrity.books_without_files, 3);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn probe_reports_unconfigured_integrations(pool: PgPool) {
    let state = test_state(pool.clone());
    let Json(probe) = diagnostics_probe(State(state)).await;

    // No integration is configured in a fresh database.
    assert_eq!(probe.integrations.len(), 4);
    assert!(probe.integrations.iter().all(|i| !i.configured));
    assert!(probe.integrations.iter().all(|i| !i.reachable));
    assert_eq!(probe.integrations[0].id, "qbittorrent");
    assert_eq!(probe.integrations[1].id, "alldebrid");
    assert_eq!(probe.integrations[2].id, "prowlarr");
    assert_eq!(probe.integrations[3].id, "telegram");

    assert_eq!(probe.missing_files.checked, 0);
    assert_eq!(probe.missing_files.missing, 0);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn probe_detects_missing_book_files(pool: PgPool) {
    let state = test_state(pool.clone());
    let lib_id = Uuid::new_v4();

    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Gon', '/libraries/gon')")
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    let book_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO books (id, library_id, kind, title, format) \
         VALUES ($1, $2, 'bd', 'Ghost', 'cbz')",
    )
    .bind(book_id)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let abs_path = format!("/stripstream-missing-{}.cbz", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) \
         VALUES ($1, $2, 'cbz', $3, 1024, NOW(), 'fp-ghost', 'ok')",
    )
    .bind(Uuid::new_v4())
    .bind(book_id)
    .bind(&abs_path)
    .execute(&pool)
    .await
    .unwrap();

    let Json(probe) = diagnostics_probe(State(state)).await;

    assert_eq!(probe.missing_files.checked, 1);
    assert_eq!(probe.missing_files.missing, 1);
    assert_eq!(probe.missing_files.samples, vec![abs_path]);
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn diagnostics_payload_never_leaks_credentials(pool: PgPool) {
    let state = test_state(pool.clone());
    let Json(report) = get_diagnostics(State(state)).await.unwrap();

    let json = serde_json::to_string(&report).unwrap().to_lowercase();
    for forbidden in ["postgres://", "password", "stl_", "bearer"] {
        assert!(
            !json.contains(forbidden),
            "diagnostics payload must not contain {forbidden:?}"
        );
    }
}

// ─── database::database_diagnostics ──────────────────────────────────────────

#[tokio::test]
async fn database_diagnostics_degrades_to_unreachable_instead_of_failing() {
    // A lazy pool that can never connect: the probe must report `reachable:false`
    // with zeroed counters rather than panicking or returning an error.
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(200))
        .connect_lazy("postgres://nobody:nobody@127.0.0.1:1/none")
        .expect("lazy pool");
    let state = test_state(pool);

    let database = super::database::database_diagnostics(&state).await;

    assert!(!database.reachable);
    assert_eq!(database.size_bytes, 0);
    assert_eq!(database.size_pretty, "0 B");
    assert_eq!(database.migrations_applied, 0);
    assert!(database.latest_migration.is_none());
    assert_eq!(database.active_connections, 0);
    assert_eq!(database.max_connections, 0);
}

// ─── services_diagnostics ────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn services_are_hidden_when_the_database_is_unreachable(pool: PgPool) {
    sqlx::query(
        "INSERT INTO service_heartbeats (service, version, started_at, last_seen_at) \
         VALUES ('api', '1.0.0', NOW(), NOW())",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Degraded database: no service is reported, even though rows exist.
    assert!(super::services_diagnostics(&pool, false).await.is_empty());

    // Healthy database: the row is surfaced.
    let services = super::services_diagnostics(&pool, true).await;
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].service, "api");
}

// ─── jobs::jobs_diagnostics ──────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn recent_failures_fall_back_to_index_job_errors(pool: PgPool) {
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Gon', '/libraries/gon')")
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    // `error_opt` is an empty string: the per-file error table must be used.
    let job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, finished_at, error_opt) \
         VALUES ($1, $2, 'scan', 'failed', NOW(), '')",
    )
    .bind(job_id)
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO index_job_errors (job_id, file_path, error_message) \
         VALUES ($1, '/libraries/gon/broken.cbz', 'per-file failure')",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();

    let diagnostics = super::jobs::jobs_diagnostics(&pool).await.unwrap();

    assert_eq!(diagnostics.recent_failures.len(), 1);
    let failure = &diagnostics.recent_failures[0];
    assert_eq!(failure.id, job_id);
    assert_eq!(failure.job_type, "scan");
    assert_eq!(failure.library_name.as_deref(), Some("Gon"));
    assert_eq!(failure.error.as_deref(), Some("per-file failure"));
}

#[sqlx::test(migrations = "../../infra/migrations")]
async fn job_breakdown_orders_busiest_types_first(pool: PgPool) {
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Gon', '/libraries/gon')")
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    // `scan` carries two active jobs…
    for status in ["pending", "running"] {
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', $3)",
        )
        .bind(Uuid::new_v4())
        .bind(lib_id)
        .bind(status)
        .execute(&pool)
        .await
        .unwrap();
    }
    // …plus a finished one, so `last_finished_at` is populated.
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status, finished_at) \
         VALUES ($1, $2, 'scan', 'success', '2026-01-01T00:00:00Z')",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    // `thumbnail_rebuild` carries only one.
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) \
         VALUES ($1, $2, 'thumbnail_rebuild', 'pending')",
    )
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let diagnostics = super::jobs::jobs_diagnostics(&pool).await.unwrap();

    let types: Vec<&str> = diagnostics
        .by_type
        .iter()
        .map(|t| t.job_type.as_str())
        .collect();
    assert_eq!(types, ["scan", "thumbnail_rebuild"]);
    assert_eq!(diagnostics.by_type[0].pending, 1);
    assert_eq!(diagnostics.by_type[0].running, 1);
    assert_eq!(
        diagnostics.by_type[0]
            .last_finished_at
            .map(|t| t.to_rfc3339()),
        Some("2026-01-01T00:00:00+00:00".to_string())
    );
    assert_eq!(diagnostics.by_type[1].pending, 1);
    assert!(diagnostics.by_type[1].last_finished_at.is_none());
}

// ─── integrity::integrity_diagnostics ────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn integrity_only_flags_regular_volume_duplicates(pool: PgPool) {
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'Gon', '/libraries/gon')")
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    let series_id = Uuid::new_v4();
    sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Gon')")
        .bind(series_id)
        .bind(lib_id)
        .execute(&pool)
        .await
        .unwrap();

    // Two HS books on the same volume: HS numbering must never look like a
    // duplicate of the regular collection.
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, kind, title, volume, volume_type) \
         VALUES ($1, $3, $4, 'bd', 'Gon HS', 1, 'hs'), \
                ($2, $3, $4, 'bd', 'Gon HS bis', 1, 'hs')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    // Two regular books with no volume number: excluded because `volume` is NULL.
    sqlx::query(
        "INSERT INTO books (id, library_id, series_id, kind, title, volume_type) \
         VALUES ($1, $3, $4, 'bd', 'Gon A', 'regular'), \
                ($2, $3, $4, 'bd', 'Gon B', 'regular')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(lib_id)
    .bind(series_id)
    .execute(&pool)
    .await
    .unwrap();

    let integrity = super::integrity::integrity_diagnostics(&pool)
        .await
        .unwrap();

    assert_eq!(integrity.duplicate_volume_groups, 0);
}

// ─── storage ─────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn thumbnail_diagnostics_counts_files_in_the_configured_directory(pool: PgPool) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("a.webp"), vec![0u8; 100]).unwrap();
    std::fs::write(directory.path().join("b.webp"), vec![0u8; 50]).unwrap();

    sqlx::query(
        "INSERT INTO app_settings (key, value) VALUES ('thumbnail', $1::jsonb) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::json!({ "directory": directory.path().to_string_lossy() }).to_string())
    .execute(&pool)
    .await
    .unwrap();

    let state = test_state(pool.clone());
    let diagnostics = super::storage::thumbnail_diagnostics(&state).await;

    assert_eq!(diagnostics.directory, directory.path().to_string_lossy());
    assert_eq!(diagnostics.file_count, 2);
    assert_eq!(diagnostics.total_size_bytes, 150);
    assert_eq!(diagnostics.total_size_pretty, "150 B");
    // No books in the database: the counters are zero, not missing.
    assert_eq!(diagnostics.missing, 0);
    assert_eq!(diagnostics.pending_analysis, 0);
}

#[tokio::test]
async fn disk_diagnostics_flags_a_missing_mount_as_unavailable() {
    let missing = format!("/stripstream-no-such-mount-{}", Uuid::new_v4());

    let disks = super::storage::disk_diagnostics(&missing, &missing).await;

    assert_eq!(disks.len(), 3);
    let ids: Vec<&str> = disks.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["libraries", "thumbnails", "image_cache"]);

    assert_eq!(disks[1].path, missing);
    assert!(!disks[1].available);
    assert_eq!(disks[1].total_bytes, 0);
    assert_eq!(disks[1].free_bytes, 0);
    assert_eq!(disks[1].used_percent, 0.0);
}

// ─── libraries_diagnostics ───────────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn libraries_diagnostics_maps_scan_and_sync_schedules(pool: PgPool) {
    let lib_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO libraries (id, name, root_path, last_scan_at, next_scan_at, \
             next_metadata_refresh_at, next_download_detection_at, next_reading_status_push_at) \
         VALUES ($1, 'Gon', '/libraries/gon', '2026-01-01T00:00:00Z', '2026-02-01T00:00:00Z', \
             '2026-02-02T00:00:00Z', '2026-02-03T00:00:00Z', '2026-02-04T00:00:00Z')",
    )
    .bind(lib_id)
    .execute(&pool)
    .await
    .unwrap();

    let libraries = super::libraries_diagnostics(&pool).await.unwrap();

    assert_eq!(libraries.len(), 1);
    let library = &libraries[0];
    assert_eq!(library.id, lib_id);
    assert_eq!(library.name, "Gon");
    assert_eq!(library.book_count, 0);
    assert_eq!(library.size_bytes, 0);
    assert_eq!(library.size_pretty, "0 B");
    assert_eq!(
        library.last_scan_at.map(|t| t.to_rfc3339()),
        Some("2026-01-01T00:00:00+00:00".to_string())
    );
    assert_eq!(
        library.next_scan_at.map(|t| t.to_rfc3339()),
        Some("2026-02-01T00:00:00+00:00".to_string())
    );
    assert!(library.next_metadata_refresh_at.is_some());
    assert!(library.next_download_detection_at.is_some());
    assert!(library.next_reading_status_push_at.is_some());
}

// ─── runtime::runtime_diagnostics ────────────────────────────────────────────

#[sqlx::test(migrations = "../../infra/migrations")]
async fn runtime_diagnostics_counts_in_flight_renders(pool: PgPool) {
    let state = test_state(pool.clone());

    let idle = super::runtime::runtime_diagnostics(&state);
    assert_eq!(idle.render_limit, 1);
    assert_eq!(idle.renders_in_flight, 0);
    assert!(idle.rss_bytes.unwrap_or(0) > 0);
    assert!(idle.rss_pretty.is_some());

    // Holding the single render permit is surfaced as one in-flight render.
    let _permit = state
        .page_render_limit
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let busy = super::runtime::runtime_diagnostics(&state);
    assert_eq!(busy.renders_in_flight, 1);
}
