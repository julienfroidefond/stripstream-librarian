//! Integration tests for the `diagnostics` module.
//!
//! The handler is called directly with the shared `test_state` helper, backed by
//! an ephemeral Postgres provisioned by `#[sqlx::test]`.

use axum::extract::State;
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
