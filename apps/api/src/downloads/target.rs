//! Shared target-path helpers for "download then import into the library" flows.
//!
//! Telegram media downloads and download-provider HTTP downloads both land a file
//! directly inside the series directory (rather than staging in `/downloads`) and
//! rely on a scan job to index it. These helpers centralize the directory
//! resolution, the filename sanitization and the rename-template rendering so both
//! flows stay consistent.

use std::path::{Path, PathBuf};

use sqlx::Row;
use tracing::info;
use uuid::Uuid;

use parsers::{detect_format, parse_metadata_fast};
use stripstream_core::paths::remap_libraries_path;

use crate::books::rename::{
    load_rename_max_volume, load_rename_templates, render_rename_filename, RenameTemplateBook,
};
use crate::downloads::import_pipeline::find_existing_series_dir;

/// Find the physical target directory for a series in a library,
/// matching existing book files first, then existing directories, then fallback to new.
pub async fn resolve_target_dir(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
) -> anyhow::Result<String> {
    // 1. Try DB: find existing book files for this series
    let any_row = sqlx::query(
        "SELECT bf.abs_path FROM book_files bf \
         JOIN books b ON b.id = bf.book_id \
         LEFT JOIN series s ON s.id = b.series_id \
         WHERE b.library_id = $1 \
           AND norm_text(s.name) = norm_text($2) \
         LIMIT 1",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    if let Some(r) = any_row {
        let abs_path: String = r.get("abs_path");
        let physical = remap_libraries_path(&abs_path);
        let parent = std::path::Path::new(&physical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(physical);
        info!("[download] Series '{}' DB match → {}", series_name, parent);
        return Ok(parent);
    }

    // 2. No DB match: look for existing directory in library root
    let lib_row = sqlx::query("SELECT root_path FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(pool)
        .await?;
    let root_path: String = lib_row.get("root_path");
    let physical_root = remap_libraries_path(&root_path);

    // The series name comes from user input (a DDL download request) and is used
    // as a directory component below, so it must not escape the library root.
    let series_component = validated_series_dir_name(series_name)?;

    let dir = find_existing_series_dir(&physical_root, series_component).unwrap_or_else(|| {
        format!(
            "{}/{}",
            physical_root.trim_end_matches('/'),
            series_component
        )
    });

    info!("[download] Series '{}' dir → {}", series_name, dir);
    Ok(dir)
}

/// The fallback target directory is `<library_root>/<series_name>`, so the series
/// name must be a single, safe path component.
fn validated_series_dir_name(series_name: &str) -> anyhow::Result<&str> {
    let name = series_name.trim();
    anyhow::ensure!(!name.is_empty(), "series name is empty");
    anyhow::ensure!(
        !name.contains("..") && !name.contains('/') && !name.contains('\\') && !name.contains('\0'),
        "series name '{series_name}' is not a valid directory name"
    );
    Ok(name)
}

pub fn sanitize_filename(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    if safe.is_empty() {
        "unknown".to_string()
    } else {
        safe
    }
}

/// Build the target filename for a downloaded file, applying the library's rename
/// templates. Returns `None` when the format is unknown, letting the caller fall
/// back to a sanitized original name.
///
/// `volume_override` carries the tome known by the caller (e.g. the provider
/// search result or the Telegram link): provider filenames like
/// `BD Astérix le Gaulois 41 …` contain a bare volume `parsers` cannot read, so
/// without it the rendered name loses the tome and every volume would collide on
/// the same file. An explicit marker in `filename` still wins.
pub async fn build_target_filename(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
    filename: &str,
    volume_override: Option<i32>,
) -> anyhow::Result<Option<String>> {
    let path = std::path::Path::new(filename);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let Some(format) = detect_format(path) else {
        return Ok(None);
    };

    let parsed = parse_metadata_fast(path, format, std::path::Path::new(""));
    let volume = parsed.volume.or(volume_override);
    let fallback_volume = volume.unwrap_or(0);
    let max_volume = load_rename_max_volume(pool, library_id, series_name, fallback_volume).await?;
    let templates = load_rename_templates(pool).await?;

    let book = RenameTemplateBook {
        title: parsed.title,
        authors: Vec::new(),
        volume,
        volume_type: parsed.volume_type.as_str().to_string(),
        publish_date: None,
        isbn: None,
        abs_path: filename.to_string(),
    };

    Ok(render_rename_filename(
        &templates,
        series_name,
        &book,
        max_volume,
        extension,
    ))
}

/// Progress is written to the DB at most every megabyte.
pub const PROGRESS_UPDATE_BYTES: i64 = 1_048_576;

/// Byte counter that reports only every [`PROGRESS_UPDATE_BYTES`].
#[derive(Debug, Default)]
pub struct DownloadProgress {
    written: i64,
    reported: i64,
}

impl DownloadProgress {
    pub const fn new() -> Self {
        Self {
            written: 0,
            reported: 0,
        }
    }

    pub const fn written(&self) -> i64 {
        self.written
    }

    /// Adds `len` bytes and returns the new total when a DB update is due.
    pub fn advance(&mut self, len: i64) -> Option<i64> {
        self.written += len;
        if self.written - self.reported >= PROGRESS_UPDATE_BYTES {
            self.reported = self.written;
            return Some(self.written);
        }
        None
    }
}

/// Where a downloaded file lands: the final path plus its in-progress `.tmp`
/// sibling.
pub struct DownloadDestination {
    pub dest_path: PathBuf,
    pub tmp_path: PathBuf,
}

/// Resolve the series directory (creating it), render the target filename and
/// return the final and temporary paths.
///
/// Any pre-existing file at the destination is left untouched: the download
/// streams to `<dest>.tmp` and only the final atomic rename replaces it, so a
/// failed or cancelled re-import never destroys the previous good file.
pub async fn prepare_destination(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    series_name: &str,
    filename: &str,
    volume_override: Option<i32>,
) -> anyhow::Result<DownloadDestination> {
    let target_dir = resolve_target_dir(pool, library_id, series_name).await?;
    tokio::fs::create_dir_all(&target_dir).await?;

    let safe_name = build_target_filename(pool, library_id, series_name, filename, volume_override)
        .await?
        .unwrap_or_else(|| sanitize_filename(filename));
    let target_dir_path = Path::new(&target_dir);
    let dest_path = target_dir_path.join(&safe_name);

    // A rendered name must not escape the series directory (a `..` or a path
    // separator slipping through a rename template).
    anyhow::ensure!(
        dest_path.parent() == Some(target_dir_path),
        "refusing to write outside the series directory: {}",
        dest_path.display()
    );

    let tmp_path = PathBuf::from(format!("{}.tmp", dest_path.display()));
    Ok(DownloadDestination {
        dest_path,
        tmp_path,
    })
}

/// Move a completed temp file into place and queue a scan so the indexer picks
/// it up. Returns the queued scan job id.
pub async fn finalize_download(
    pool: &sqlx::PgPool,
    library_id: Uuid,
    tmp_path: &Path,
    dest_path: &Path,
) -> anyhow::Result<Uuid> {
    tokio::fs::rename(tmp_path, dest_path).await?;
    queue_scan_job(pool, library_id).await
}

/// Queue a `scan` job for a library so freshly written files get indexed.
pub async fn queue_scan_job(pool: &sqlx::PgPool, library_id: Uuid) -> anyhow::Result<Uuid> {
    let scan_job_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'pending')",
    )
    .bind(scan_job_id)
    .bind(library_id)
    .execute(pool)
    .await?;
    Ok(scan_job_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_library(pool: &sqlx::PgPool, template: &str) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("test")
            .bind(format!("/libraries/{}", library_id))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ('rename_format', $1) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(serde_json::json!(template))
        .execute(pool)
        .await
        .unwrap();
        library_id
    }

    /// A provider filename with a bare volume (`… 41 …`) is not understood by
    /// `parsers`; the caller-supplied volume must fill the gap so the file is not
    /// written without its tome.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn volume_override_fills_missing_bare_volume(pool: sqlx::PgPool) {
        let library_id = seed_library(&pool, "{series_name} - T{volume_padded}").await;

        let name = build_target_filename(
            &pool,
            library_id,
            "Astérix",
            "BD Astérix le Gaulois 41 Astérix en Lusitanie – René Goscinny & Albert Uderzo.pdf",
            Some(41),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(name, "Astérix - T41.pdf");
    }

    /// An explicit `T03` marker in the filename still wins over a stale override.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn explicit_volume_marker_wins_over_override(pool: sqlx::PgPool) {
        let library_id = seed_library(&pool, "{series_name} - T{volume_padded}").await;

        let name =
            build_target_filename(&pool, library_id, "Astérix", "Astérix - T03.cbz", Some(99))
                .await
                .unwrap()
                .unwrap();

        assert_eq!(name, "Astérix - T3.cbz");
    }

    /// `series_name` is user input; a traversal component must be rejected before
    /// it is joined to the library root.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn resolve_target_dir_rejects_series_name_traversal(pool: sqlx::PgPool) {
        let library_id = seed_library(&pool, "{series_name}").await;

        assert!(
            resolve_target_dir(&pool, library_id, "../../etc")
                .await
                .is_err(),
            "a series name must not escape the library root"
        );
    }

    /// A re-import must not delete the previous good file up front: the transfer
    /// streams to a `.tmp` sibling and only the final rename replaces the target.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn prepare_destination_keeps_an_existing_file(pool: sqlx::PgPool) {
        let root = std::env::temp_dir().join(format!("stripstream-target-{}", Uuid::new_v4()));
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind("test")
            .bind(root.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO app_settings (key, value) VALUES ('rename_format', $1) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(serde_json::json!("{series_name} - T{volume_padded}"))
        .execute(&pool)
        .await
        .unwrap();

        let first = prepare_destination(&pool, library_id, "Astérix", "Astérix - T01.cbz", None)
            .await
            .unwrap();
        tokio::fs::write(&first.dest_path, b"GOOD").await.unwrap();

        let second = prepare_destination(&pool, library_id, "Astérix", "Astérix - T01.cbz", None)
            .await
            .unwrap();

        assert_eq!(second.dest_path, first.dest_path);
        assert_eq!(
            tokio::fs::read(&second.dest_path).await.unwrap(),
            b"GOOD",
            "preparing a re-download must not delete the existing file"
        );

        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[test]
    fn progress_reports_at_most_every_megabyte() {
        let mut progress = DownloadProgress::new();
        assert_eq!(progress.advance(PROGRESS_UPDATE_BYTES - 1), None);
        assert_eq!(progress.advance(1), Some(PROGRESS_UPDATE_BYTES));
        assert_eq!(progress.advance(10), None);
        assert_eq!(progress.written(), PROGRESS_UPDATE_BYTES + 10);
        assert_eq!(
            progress.advance(PROGRESS_UPDATE_BYTES),
            Some(2 * PROGRESS_UPDATE_BYTES + 10)
        );
    }
}
