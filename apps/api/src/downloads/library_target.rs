//! Resolve the physical target directory (and optional naming reference) for a
//! series inside a library.
//!
//! Every "download then import" flow resolves the series directory the same
//! way: prefer a directory already known from the database, then an existing
//! directory matching the series name case/accent-insensitively, then a new
//! `<library_root>/<series_name>` directory. The last fallback uses the series
//! name as a path component, so it is validated here — never at call sites.

use std::path::Path;

use sqlx::Row;
use tracing::info;
use uuid::Uuid;

use parsers::fold_accents;
use stripstream_core::paths::LibraryRoot;

/// A resolved destination directory for a series, plus the optional book file
/// used as a naming reference for imported volumes.
#[derive(Debug)]
pub(super) struct LibraryTarget {
    pub(super) directory: String,
    pub(super) reference: Option<LibraryReference>,
}

/// An existing book file belonging to the series, used to preserve the naming
/// scheme of new volumes.
#[derive(Debug)]
pub(super) struct LibraryReference {
    pub(super) abs_path: String,
    pub(super) volume: i32,
}

/// Resolve the directory a series should be imported into.
///
/// A database match wins: its parent directory is authoritative even when the
/// series name itself would not be a safe new directory component. Only the
/// fallback path uses `series_name` as a component, and that path is validated.
pub(super) async fn resolve(
    pool: &sqlx::PgPool,
    library_root: &LibraryRoot,
    library_id: Uuid,
    series_name: &str,
) -> anyhow::Result<LibraryTarget> {
    // 1. Try DB: find an existing book file for this series. The highest volume
    // is the best naming reference; NULL volumes still locate the directory.
    let row = sqlx::query(
        "SELECT bf.abs_path, b.volume
         FROM book_files bf
         JOIN books b ON b.id = bf.book_id
         LEFT JOIN series s ON s.id = b.series_id
         WHERE b.library_id = $1
           AND norm_text(s.name) = norm_text($2)
         ORDER BY b.volume DESC NULLS LAST
         LIMIT 1",
    )
    .bind(library_id)
    .bind(series_name)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        let abs_path: String = row.get("abs_path");
        let volume: Option<i32> = row.get("volume");
        let physical = library_root.resolve(&abs_path)?.to_local().to_string();
        let parent = Path::new(&physical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(physical);
        info!("[download] Series '{}' DB match → {}", series_name, parent);
        return Ok(LibraryTarget {
            directory: parent,
            reference: volume.map(|volume| LibraryReference { abs_path, volume }),
        });
    }

    // 2. No DB match: look for an existing directory in the library root.
    let physical_root = resolve_physical_root(pool, library_root, library_id).await?;

    // The series name is user input and becomes a directory component below.
    let series_component = validated_series_dir_name(series_name)?;

    let directory =
        find_existing_series_dir(&physical_root, series_component).unwrap_or_else(|| {
            format!(
                "{}/{}",
                physical_root.trim_end_matches('/'),
                series_component
            )
        });

    info!("[download] Series '{}' dir → {}", series_name, directory);
    Ok(LibraryTarget {
        directory,
        reference: None,
    })
}

/// Resolve the physical root of a library, used as the base for interpreting
/// content paths. Kept here so callers do not hand-join the root themselves.
pub(super) async fn resolve_physical_root(
    pool: &sqlx::PgPool,
    library_root: &LibraryRoot,
    library_id: Uuid,
) -> anyhow::Result<String> {
    let root_path: String = sqlx::query_scalar("SELECT root_path FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(pool)
        .await?;
    Ok(library_root.resolve(&root_path)?.to_local().to_string())
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

/// Find an existing directory in `root` whose name matches `series_name`
/// case-insensitively and accent-insensitively (e.g. "les géants" matches "les geants").
fn find_existing_series_dir(root: &str, series_name: &str) -> Option<String> {
    let target_norm = fold_accents(&series_name.to_lowercase());
    let entries = std::fs::read_dir(root).ok()?;
    let mut best: Option<(String, bool)> = None; // (path, is_exact_case_match)
    for entry in entries.flatten() {
        if !entry.file_type().ok().is_some_and(|t| t.is_dir()) {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let name_lower = name_str.to_lowercase();
        let name_norm = fold_accents(&name_lower);
        if name_norm == target_norm {
            let path = entry.path().to_string_lossy().into_owned();
            let exact = name_lower == series_name.to_lowercase();
            info!(
                "[IMPORT] Found existing directory (normalized match): {} (exact={})",
                path, exact
            );
            // Prefer exact case match over accent-stripped match
            if exact || best.is_none() {
                best = Some((path, exact));
            }
        }
    }
    best.map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_library_root() -> LibraryRoot {
        LibraryRoot::new(stripstream_core::paths::LIBRARIES_DB_ROOT)
    }

    async fn seed_library(pool: &sqlx::PgPool) -> (Uuid, String) {
        let library_id = Uuid::new_v4();
        let root_path = format!("/libraries/{library_id}");
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(library_id)
            .bind(format!("lib_{library_id}"))
            .bind(&root_path)
            .execute(pool)
            .await
            .unwrap();
        (library_id, root_path)
    }

    async fn seed_book(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        series_name: &str,
        volume: Option<i32>,
        abs_path: &str,
    ) -> Uuid {
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind(series_name)
            .execute(pool)
            .await
            .unwrap();

        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, series_id, title, kind, format, volume, volume_type) \
             VALUES ($1, $2, $3, $4, 'comic', 'cbz', $5, 'regular')",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(series_id)
        .bind(series_name)
        .bind(volume)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, 'cbz', $3, 1, NOW(), $4, 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(abs_path)
        .bind(format!("fp_{book_id}"))
        .execute(pool)
        .await
        .unwrap();

        series_id
    }

    // ─── find_existing_series_dir (uses temp dirs) ───────────────────────

    #[test]
    fn find_existing_series_dir_exact_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("One Piece")).unwrap();

        let result = find_existing_series_dir(dir.path().to_str().unwrap(), "One Piece");
        assert!(result.is_some());
        assert!(result.unwrap().contains("One Piece"));
    }

    #[test]
    fn find_existing_series_dir_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("one piece")).unwrap();

        let result = find_existing_series_dir(dir.path().to_str().unwrap(), "One Piece");
        assert!(result.is_some());
    }

    #[test]
    fn find_existing_series_dir_accent_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("les geants")).unwrap();

        let result = find_existing_series_dir(dir.path().to_str().unwrap(), "les géants");
        assert!(result.is_some());
    }

    #[test]
    fn find_existing_series_dir_no_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Naruto")).unwrap();

        let result = find_existing_series_dir(dir.path().to_str().unwrap(), "One Piece");
        assert!(result.is_none());
    }

    #[test]
    fn find_existing_series_dir_prefers_exact_case() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Les Geants")).unwrap();
        std::fs::create_dir(dir.path().join("les géants")).unwrap();

        // When searching for "les géants", should prefer the exact case match
        let result = find_existing_series_dir(dir.path().to_str().unwrap(), "les géants");
        assert!(result.is_some());
        assert!(result.unwrap().contains("les géants"));
    }

    // ─── validated_series_dir_name ───────────────────────────────────────

    #[test]
    fn validated_series_dir_name_accepts_and_trims_a_plain_name() {
        assert_eq!(validated_series_dir_name("  Astérix  ").unwrap(), "Astérix");
    }

    #[test]
    fn validated_series_dir_name_rejects_unsafe_components() {
        for name in ["", "   ", "..", "../etc", "a/b", "a\\b", "a\0b"] {
            assert!(
                validated_series_dir_name(name).is_err(),
                "{name:?} must not be used as a directory component"
            );
        }
    }

    // ─── resolve (sqlx::test) ────────────────────────────────────────────

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn resolve_rejects_traversal_without_db_match(pool: sqlx::PgPool) {
        let (library_id, _) = seed_library(&pool).await;

        let error = resolve(&pool, &test_library_root(), library_id, "../../etc")
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains("not a valid directory name"),
            "unexpected error: {error}"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn resolve_returns_db_reference_and_directory(pool: sqlx::PgPool) {
        let (library_id, root_path) = seed_library(&pool).await;
        let abs_path = format!("{root_path}/Amulet/Amulet - T2.cbz");
        seed_book(&pool, library_id, "Amulet", Some(2), &abs_path).await;

        let target = resolve(&pool, &test_library_root(), library_id, "amulet")
            .await
            .unwrap();

        assert_eq!(target.directory, format!("{root_path}/Amulet"));
        let reference = target.reference.expect("volume 2 is a naming reference");
        assert_eq!(reference.abs_path, abs_path);
        assert_eq!(reference.volume, 2);
    }

    /// A slash in the series name is only unsafe for the new-directory fallback.
    /// When the library already owns a book, its parent directory is authoritative.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn resolve_allows_db_match_with_slash_in_series_name(pool: sqlx::PgPool) {
        let (library_id, root_path) = seed_library(&pool).await;
        let abs_path = format!("{root_path}/Fate Stay Night/Fate Stay Night - T1.cbz");
        seed_book(&pool, library_id, "Fate/Stay Night", Some(1), &abs_path).await;

        let target = resolve(&pool, &test_library_root(), library_id, "Fate/Stay Night")
            .await
            .unwrap();

        assert_eq!(target.directory, format!("{root_path}/Fate Stay Night"));
        assert_eq!(target.reference.expect("volume 1").volume, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn resolve_uses_null_volume_directory_without_reference(pool: sqlx::PgPool) {
        let (library_id, root_path) = seed_library(&pool).await;
        let abs_path = format!("{root_path}/One Shot/One Shot.cbz");
        seed_book(&pool, library_id, "One Shot", None, &abs_path).await;

        let target = resolve(&pool, &test_library_root(), library_id, "One Shot")
            .await
            .unwrap();

        assert_eq!(target.directory, format!("{root_path}/One Shot"));
        assert!(
            target.reference.is_none(),
            "a book without a volume cannot be a naming reference"
        );
    }
}
