//! Page cache, thumbnails and filesystem capacity.

use super::{format_bytes, DiskDiagnostics, ThumbnailDiagnostics};
use crate::state::AppState;

// ─── Thumbnails ──────────────────────────────────────────────────────────────

/// Thumbnail footprint on disk plus the book counters that explain it.
///
/// Never fails: when the directory walk or the counter query fails the page
/// still renders, with zeroes for the missing part.
pub(super) async fn thumbnail_diagnostics(state: &AppState) -> ThumbnailDiagnostics {
    let directory = crate::settings::thumbnail_directory(&state.pool).await;
    let walk_dir = directory.clone();
    let (total_size_bytes, file_count) = tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&walk_dir);
        if path.exists() {
            crate::settings::compute_dir_stats(path)
        } else {
            (0, 0)
        }
    })
    .await
    .unwrap_or((0, 0));

    let (missing, pending_analysis) = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE thumbnail_path IS NULL) AS missing,
            COUNT(*) FILTER (WHERE page_count IS NULL) AS pending_analysis
        FROM books
        "#,
    )
    .fetch_one(&state.pool)
    .await
    .map(|row| {
        use sqlx::Row;
        (
            row.get::<i64, _>("missing"),
            row.get::<i64, _>("pending_analysis"),
        )
    })
    .unwrap_or((0, 0));

    ThumbnailDiagnostics {
        total_size_bytes,
        total_size_pretty: format_bytes(total_size_bytes),
        file_count,
        directory,
        missing,
        pending_analysis,
    }
}

// ─── Filesystems ─────────────────────────────────────────────────────────────

/// Capacity of the volumes the services write to.
///
/// `statvfs` is blocking, so every path is measured in one `spawn_blocking`
/// task. Missing mounts come back as `available: false`, never as an error.
pub(super) async fn disk_diagnostics(thumbnail_dir: &str, cache_dir: &str) -> Vec<DiskDiagnostics> {
    let targets = vec![
        ("libraries".to_string(), libraries_root()),
        ("thumbnails".to_string(), thumbnail_dir.to_string()),
        ("image_cache".to_string(), cache_dir.to_string()),
    ];

    tokio::task::spawn_blocking(move || {
        targets
            .into_iter()
            .map(|(id, path)| {
                let space = stripstream_core::disk::space(&path);
                DiskDiagnostics {
                    id,
                    path: space.path,
                    available: space.available,
                    total_bytes: space.total_bytes,
                    free_bytes: space.free_bytes,
                    used_percent: space.used_percent,
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default()
}

/// Host path of the `libraries/` volume, or its container default.
fn libraries_root() -> String {
    std::env::var("LIBRARIES_ROOT_PATH")
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/libraries".to_string())
}
