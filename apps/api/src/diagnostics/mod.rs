//! Aggregated health diagnostics for the Backoffice "Diagnostic / Santé" page.
//!
//! Everything here is read-only and admin-scoped. The payload deliberately
//! contains no connection string, credential or token: only counts, sizes,
//! versions and the cache/thumbnail directories that Settings already exposes.
//!
//! The module is split by subsystem so each probe stays small:
//!
//! | Module      | Covers                                                    |
//! |-------------|-----------------------------------------------------------|
//! | `database`  | connection probe, size, migrations, pool usage            |
//! | `storage`   | page cache, thumbnails and filesystem capacity             |
//! | `jobs`      | queue depth, stuck jobs, failures, per-type breakdown      |
//! | `integrity` | DB consistency counters (missing files, orphans, dupes)    |
//! | `runtime`   | process memory, file descriptors, rendering slots         |
//! | `probe`     | on-demand integration / missing-file probe (`/diagnostics/probe`) |
//!
//! Two design rules:
//!
//! - **Never fail the page.** A probe that times out yields zeroes and an
//!   `available: false` flag instead of a 500.
//! - **Never leak secrets.** Error strings from external services are reduced
//!   to a status code before being returned.

mod database;
mod integrity;
mod jobs;
pub(crate) mod probe;
mod runtime;
mod storage;

use std::sync::atomic::Ordering;

use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

pub use probe::{diagnostics_probe, DiagnosticsProbe, IntegrationDiagnostics, MissingFilesReport};

/// How long the database probe may take before it is reported as unreachable.
const DB_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

pub fn diagnostics_routes() -> Router<AppState> {
    Router::new()
        .route("/diagnostics", get(get_diagnostics))
        .route("/diagnostics/probe", get(diagnostics_probe))
}

// ─── Payload ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DiagnosticsResponse {
    /// API crate version (`CARGO_PKG_VERSION`).
    pub version: String,
    /// Seconds since the API process started.
    pub uptime_seconds: u64,
    /// Liveness of every service sharing the database (API, indexer).
    pub services: Vec<ServiceDiagnostics>,
    pub database: DatabaseDiagnostics,
    pub runtime: RuntimeDiagnostics,
    /// Filesystem capacity of the volumes the services write to.
    pub disks: Vec<DiskDiagnostics>,
    pub cache: CacheDiagnostics,
    pub thumbnails: ThumbnailDiagnostics,
    pub jobs: JobDiagnostics,
    pub integrity: IntegrityDiagnostics,
    pub libraries: Vec<LibraryDiagnostics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServiceDiagnostics {
    /// Stable identifier: `api` or `indexer`.
    pub service: String,
    pub version: String,
    pub online: bool,
    /// Seconds since the last heartbeat.
    pub seconds_since_seen: i64,
    /// Seconds since the process itself started (from the heartbeat row).
    pub uptime_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DatabaseDiagnostics {
    pub reachable: bool,
    /// Round-trip time of the probe query, in milliseconds.
    pub latency_ms: u64,
    pub size_bytes: i64,
    pub size_pretty: String,
    pub migrations_applied: i64,
    pub latest_migration: Option<String>,
    pub active_connections: i64,
    pub max_connections: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct RuntimeDiagnostics {
    /// Resident set size of the API process, when it can be read.
    pub rss_bytes: Option<u64>,
    pub rss_pretty: Option<String>,
    /// Open file descriptors (Linux only, `None` elsewhere).
    pub open_fds: Option<u64>,
    /// Page renders currently in flight.
    pub renders_in_flight: usize,
    /// Configured `concurrent_renders` limit.
    pub render_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DiskDiagnostics {
    /// Stable identifier: `libraries`, `thumbnails` or `image_cache`.
    pub id: String,
    pub path: String,
    pub available: bool,
    pub total_bytes: u64,
    pub free_bytes: u64,
    /// Share of the volume in use, rounded to one decimal.
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CacheDiagnostics {
    pub memory_size_mb: f64,
    pub memory_page_count: usize,
    pub memory_max_size_mb: f64,
    pub disk_size_mb: f64,
    pub disk_file_count: u64,
    pub directory: String,
    pub hits: u64,
    pub misses: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ThumbnailDiagnostics {
    pub total_size_bytes: u64,
    pub total_size_pretty: String,
    pub file_count: u64,
    pub directory: String,
    /// Books whose thumbnail has not been generated yet.
    pub missing: i64,
    /// Books discovered but not analysed yet (`page_count IS NULL`).
    pub pending_analysis: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct JobDiagnostics {
    pub total: i64,
    pub pending: i64,
    pub running: i64,
    pub failed_last_24h: i64,
    /// Jobs pending for more than 30 minutes: the scheduler's `NOT EXISTS`
    /// guard treats them as in-flight and plans nothing new, so a stuck job
    /// silently freezes the library.
    pub stale_pending: i64,
    /// Age of the oldest pending job, in seconds.
    pub oldest_pending_seconds: Option<i64>,
    /// Per-type breakdown, busiest types first.
    pub by_type: Vec<JobTypeDiagnostics>,
    pub recent_failures: Vec<FailedJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobTypeDiagnostics {
    pub job_type: String,
    pub pending: i64,
    pub running: i64,
    pub failed_last_24h: i64,
    pub last_finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FailedJob {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub job_type: String,
    pub library_name: Option<String>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct IntegrityDiagnostics {
    /// Books with no `book_files` row at all.
    pub books_without_files: i64,
    /// Books analysed successfully but reporting zero pages.
    pub books_zero_pages: i64,
    /// Series with no metadata link (any provider).
    pub series_without_metadata: i64,
    /// `(library, series, volume)` groups holding more than one book.
    pub duplicate_volume_groups: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LibraryDiagnostics {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub book_count: i64,
    pub pending_analysis: i64,
    pub missing_thumbnails: i64,
    pub size_bytes: i64,
    pub size_pretty: String,
    pub last_scan_at: Option<DateTime<Utc>>,
    pub next_scan_at: Option<DateTime<Utc>>,
    pub last_metadata_refresh_at: Option<DateTime<Utc>>,
    pub next_metadata_refresh_at: Option<DateTime<Utc>>,
    pub last_download_detection_at: Option<DateTime<Utc>>,
    pub next_download_detection_at: Option<DateTime<Utc>>,
    pub last_reading_status_push_at: Option<DateTime<Utc>>,
    pub next_reading_status_push_at: Option<DateTime<Utc>>,
}

// ─── Handler ─────────────────────────────────────────────────────────────────

/// Aggregated health of the API, database, caches and indexer jobs.
#[utoipa::path(
    get,
    path = "/diagnostics",
    tag = "diagnostics",
    responses(
        (status = 200, body = DiagnosticsResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("Bearer" = []))
)]
pub async fn get_diagnostics(
    State(state): State<AppState>,
) -> Result<Json<DiagnosticsResponse>, ApiError> {
    let database = database::database_diagnostics(&state).await;
    let services = services_diagnostics(&state.pool, database.reachable).await;

    // Cache stats only read in-memory state and the filesystem, so they stay
    // available even when the database probe fails.
    let cache_stats = crate::settings::get_cache_stats(State(state.clone()))
        .await?
        .0;

    // Degrade gracefully: a degraded database must still render a diagnostics
    // page, not a 500.
    let thumbnails = storage::thumbnail_diagnostics(&state).await;
    let jobs = jobs::jobs_diagnostics(&state.pool)
        .await
        .unwrap_or_default();
    let integrity = integrity::integrity_diagnostics(&state.pool)
        .await
        .unwrap_or_default();
    let libraries = libraries_diagnostics(&state.pool).await.unwrap_or_default();
    let disks = storage::disk_diagnostics(&thumbnails.directory, &cache_stats.directory).await;
    let runtime = runtime::runtime_diagnostics(&state);

    Ok(Json(DiagnosticsResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.started_at.elapsed().as_secs(),
        services,
        database,
        runtime,
        disks,
        cache: CacheDiagnostics {
            memory_size_mb: cache_stats.memory_size_mb,
            memory_page_count: cache_stats.memory_page_count,
            memory_max_size_mb: cache_stats.memory_max_size_mb,
            disk_size_mb: cache_stats.total_size_mb,
            disk_file_count: cache_stats.file_count,
            directory: cache_stats.directory,
            hits: state.metrics.page_cache_hits.load(Ordering::Relaxed),
            misses: state.metrics.page_cache_misses.load(Ordering::Relaxed),
        },
        thumbnails,
        jobs,
        integrity,
        libraries,
    }))
}

// ─── Probes ──────────────────────────────────────────────────────────────────

/// Liveness of every service with a heartbeat row.
///
/// The API always appears (an unreachable database yields an empty list, and
/// the UI still shows the API version from the payload).
async fn services_diagnostics(pool: &sqlx::PgPool, reachable: bool) -> Vec<ServiceDiagnostics> {
    if !reachable {
        return Vec::new();
    }

    let now = Utc::now();

    stripstream_core::heartbeat::load_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|heartbeat| ServiceDiagnostics {
            online: heartbeat.is_online(now),
            seconds_since_seen: heartbeat.seconds_since_seen(now),
            uptime_seconds: (now - heartbeat.started_at).num_seconds().max(0),
            version: heartbeat.version,
            service: heartbeat.service,
        })
        .collect()
}

async fn libraries_diagnostics(
    pool: &sqlx::PgPool,
) -> Result<Vec<LibraryDiagnostics>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            l.id,
            l.name,
            l.enabled,
            l.last_scan_at,
            l.next_scan_at,
            l.last_metadata_refresh_at,
            l.next_metadata_refresh_at,
            l.last_download_detection_at,
            l.next_download_detection_at,
            l.last_reading_status_push_at,
            l.next_reading_status_push_at,
            COALESCE(books.book_count, 0) AS book_count,
            COALESCE(books.pending_analysis, 0) AS pending_analysis,
            COALESCE(books.missing_thumbnails, 0) AS missing_thumbnails,
            COALESCE(files.size_bytes, 0) AS size_bytes
        FROM libraries l
        LEFT JOIN LATERAL (
            SELECT
                COUNT(*) AS book_count,
                COUNT(*) FILTER (WHERE b.page_count IS NULL) AS pending_analysis,
                COUNT(*) FILTER (WHERE b.thumbnail_path IS NULL) AS missing_thumbnails
            FROM books b
            WHERE b.library_id = l.id
        ) books ON TRUE
        LEFT JOIN LATERAL (
            SELECT SUM(bf.size_bytes)::bigint AS size_bytes
            FROM book_files bf
            JOIN books b ON b.id = bf.book_id
            WHERE b.library_id = l.id
        ) files ON TRUE
        ORDER BY l.name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let size_bytes: i64 = row.get("size_bytes");
            LibraryDiagnostics {
                id: row.get("id"),
                name: row.get("name"),
                enabled: row.get("enabled"),
                book_count: row.get("book_count"),
                pending_analysis: row.get("pending_analysis"),
                missing_thumbnails: row.get("missing_thumbnails"),
                size_bytes,
                size_pretty: format_bytes(size_bytes.max(0) as u64),
                last_scan_at: row.get("last_scan_at"),
                next_scan_at: row.get("next_scan_at"),
                last_metadata_refresh_at: row.get("last_metadata_refresh_at"),
                next_metadata_refresh_at: row.get("next_metadata_refresh_at"),
                last_download_detection_at: row.get("last_download_detection_at"),
                next_download_detection_at: row.get("next_download_detection_at"),
                last_reading_status_push_at: row.get("last_reading_status_push_at"),
                next_reading_status_push_at: row.get("next_reading_status_push_at"),
            }
        })
        .collect())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Human-readable byte size (`0 B`, `1023 B`, `1.5 MB`…).
pub(crate) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
#[path = "../tests/diagnostics.rs"]
mod tests;
