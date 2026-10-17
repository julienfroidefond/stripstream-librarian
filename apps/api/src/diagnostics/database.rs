//! Database probe: reachability, size, migrations and pool usage.

use sqlx::Row;

use super::{DatabaseDiagnostics, DB_PROBE_TIMEOUT};
use crate::state::AppState;

pub(super) async fn database_diagnostics(state: &AppState) -> DatabaseDiagnostics {
    let started = std::time::Instant::now();
    let probe = tokio::time::timeout(DB_PROBE_TIMEOUT, async {
        sqlx::query(
            r#"
            SELECT
                pg_database_size(current_database()) AS size_bytes,
                (SELECT COUNT(*) FROM _sqlx_migrations) AS migrations_applied,
                (SELECT version || ' · ' || description
                   FROM _sqlx_migrations
                  ORDER BY version DESC
                  LIMIT 1) AS latest_migration,
                (SELECT COUNT(*) FROM pg_stat_activity
                  WHERE datname = current_database()) AS active_connections,
                current_setting('max_connections')::int AS max_connections
            "#,
        )
        .fetch_one(&state.pool)
        .await
    })
    .await;

    let latency_ms = started.elapsed().as_millis() as u64;

    match probe {
        Ok(Ok(row)) => {
            let size_bytes: i64 = row.get("size_bytes");
            DatabaseDiagnostics {
                reachable: true,
                latency_ms,
                size_bytes,
                size_pretty: super::format_bytes(size_bytes.max(0) as u64),
                migrations_applied: row.get("migrations_applied"),
                latest_migration: row.get("latest_migration"),
                active_connections: row.get("active_connections"),
                max_connections: row.get("max_connections"),
            }
        }
        Ok(Err(_)) | Err(_) => DatabaseDiagnostics {
            reachable: false,
            latency_ms,
            size_bytes: 0,
            size_pretty: super::format_bytes(0),
            migrations_applied: 0,
            latest_migration: None,
            active_connections: 0,
            max_connections: 0,
        },
    }
}
