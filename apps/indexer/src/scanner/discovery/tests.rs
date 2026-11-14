//! Discovery walk: stale deletions on partial walks, per-directory mtime skip, path encoding.

use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::scan_library_discovery;
use crate::scanner::JobStats;
use crate::AppState;

struct Fixture {
    _dir: tempfile::TempDir,
    state: AppState,
    root: PathBuf,
    library_id: Uuid,
    job_id: Uuid,
}

impl Fixture {
    /// An empty library `lib` mounted at `<tmp>/lib` (DB prefix `/libraries/lib`).
    async fn new(pool: &sqlx::PgPool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let state = AppState {
            pool: pool.clone(),
            library_root: stripstream_core::paths::LibraryRoot::new(dir.path()),
        };
        let library_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'lib', '/libraries/lib')",
        )
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'running')",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();
        Self {
            _dir: dir,
            state,
            root,
            library_id,
            job_id,
        }
    }

    fn write_file(&self, rel: &str) -> PathBuf {
        let path = self.root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not really a zip").unwrap();
        path
    }

    /// A book on disk at `rel`, already known (with its current fingerprint) in the DB.
    async fn known_book(&self, rel: &str) -> Uuid {
        let path = self.write_file(rel);
        let meta = std::fs::metadata(&path).unwrap();
        let mtime: DateTime<Utc> = meta.modified().unwrap().into();
        let fingerprint = crate::utils::compute_fingerprint(&path, meta.len(), &mtime).unwrap();
        let book_id = Uuid::new_v4();
        let title = Path::new(rel).file_stem().unwrap().to_str().unwrap();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format, analysis_status) \
             VALUES ($1, $2, $3, 'comic', 'cbz', 'complete')",
        )
        .bind(book_id)
        .bind(self.library_id)
        .bind(title)
        .execute(&self.state.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, $3, 'cbz', $4, $5, $6, 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(format!("/libraries/lib/{rel}"))
        .bind(meta.len() as i64)
        .bind(mtime)
        .bind(fingerprint)
        .execute(&self.state.pool)
        .await
        .unwrap();
        book_id
    }

    /// Store `rel_dir`'s mtime as seen by a previous scan: in the future (unchanged) or in
    /// the past (changed since).
    async fn store_dir_mtime(&self, rel_dir: &str, unchanged: bool) {
        let offset = if unchanged {
            chrono::Duration::days(1)
        } else {
            chrono::Duration::days(-365 * 20)
        };
        sqlx::query(
            "INSERT INTO directory_mtimes (library_id, dir_path, mtime) VALUES ($1, $2, $3)",
        )
        .bind(self.library_id)
        .bind(format!("/libraries/lib/{rel_dir}"))
        .bind(Utc::now() + offset)
        .execute(&self.state.pool)
        .await
        .unwrap();
    }

    async fn run(&self) -> JobStats {
        let mut stats = JobStats {
            scanned_files: 0,
            indexed_files: 0,
            removed_files: 0,
            errors: 0,
            warnings: 0,
            new_series: 0,
            new_series_names: Vec::new(),
            new_book_titles: Vec::new(),
        };
        let mut processed = 0;
        scan_library_discovery(
            &self.state,
            self.job_id,
            self.library_id,
            &self.root,
            &mut stats,
            &mut processed,
            0,
            false,
        )
        .await
        .unwrap();
        stats
    }

    async fn book_titles(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT title FROM books WHERE library_id = $1 ORDER BY title")
            .bind(self.library_id)
            .fetch_all(&self.state.pool)
            .await
            .unwrap()
    }

    async fn stored_dir_mtimes(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM directory_mtimes WHERE library_id = $1")
            .bind(self.library_id)
            .fetch_one(&self.state.pool)
            .await
            .unwrap()
    }
}

/// An unreadable subdirectory is skipped by the walk: the books it holds are not in `seen`,
/// yet still on disk. No stale deletion may happen on such a partial walk.
#[cfg(unix)]
#[sqlx::test(migrations = "../../infra/migrations")]
async fn partial_walk_never_deletes_books(pool: sqlx::PgPool) {
    use std::os::unix::fs::PermissionsExt;

    let f = Fixture::new(&pool).await;
    f.known_book("Open/Open - T01.cbz").await;
    f.known_book("Locked/Locked - T01.cbz").await;
    f.known_book("Gone/Gone - T01.cbz").await;
    std::fs::remove_file(f.root.join("Gone/Gone - T01.cbz")).unwrap();

    let locked = f.root.join("Locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&locked).is_ok() {
        // Running as root: permissions are not enforced, the scenario cannot be built.
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let stats = f.run().await;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        f.book_titles().await,
        vec!["Gone - T01", "Locked - T01", "Open - T01"],
        "no deletion at all while part of the tree was unexplored"
    );
    assert_eq!(stats.removed_files, 0);
    assert!(stats.warnings > 0, "the partial walk is reported");
    assert_eq!(
        f.stored_dir_mtimes().await,
        0,
        "directory mtimes are not saved from a partial walk"
    );

    // Once the tree is readable again, the really missing book is removed.
    let stats = f.run().await;
    assert_eq!(stats.removed_files, 1);
    assert_eq!(f.book_titles().await, vec!["Locked - T01", "Open - T01"]);
}

/// `Publisher/` is unchanged but `Publisher/Series/` got a new file: its own mtime changed,
/// so the new book is indexed (a subdirectory never inherits its parent's skip).
#[sqlx::test(migrations = "../../infra/migrations")]
async fn subdirectory_compares_its_own_mtime(pool: sqlx::PgPool) {
    let f = Fixture::new(&pool).await;
    f.known_book("Publisher/Series/Series - T01.cbz").await;
    f.write_file("Publisher/Series/Series - T02.cbz");
    f.store_dir_mtime("Publisher", true).await;
    f.store_dir_mtime("Publisher/Series", false).await;

    let stats = f.run().await;

    assert_eq!(stats.indexed_files, 1);
    assert_eq!(f.book_titles().await, vec!["Series - T01", "Series - T02"]);
}

/// The skip still applies to the files directly inside an unchanged directory: an unknown
/// file there is not inserted, a known one is kept.
#[sqlx::test(migrations = "../../infra/migrations")]
async fn unchanged_directory_skips_only_its_own_files(pool: sqlx::PgPool) {
    let f = Fixture::new(&pool).await;
    f.known_book("Series/Series - T01.cbz").await;
    f.write_file("Series/Series - T02.cbz");
    f.store_dir_mtime("Series", true).await;

    let stats = f.run().await;

    assert_eq!((stats.indexed_files, stats.removed_files), (0, 0));
    assert_eq!(f.book_titles().await, vec!["Series - T01"]);
}

/// A file whose name is not valid UTF-8 is skipped with a warning instead of being stored
/// under a lossy (unresolvable) path. Some filesystems (APFS) refuse such names: the test
/// is then a no-op.
#[cfg(unix)]
#[sqlx::test(migrations = "../../infra/migrations")]
async fn non_utf8_paths_are_skipped_with_a_warning(pool: sqlx::PgPool) {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let f = Fixture::new(&pool).await;
    f.write_file("Series/Series - T01.cbz");
    let bad_dir = f.root.join(OsStr::from_bytes(b"Bad \xff Dir"));
    let bad_file = f
        .root
        .join("Series")
        .join(OsStr::from_bytes(b"Series \xfe T02.cbz"));
    if std::fs::create_dir(&bad_dir).is_err() || std::fs::write(&bad_file, b"zip").is_err() {
        return;
    }
    std::fs::write(bad_dir.join("Inner - T01.cbz"), b"zip").unwrap();

    let stats = f.run().await;

    assert_eq!(f.book_titles().await, vec!["Series - T01"]);
    assert_eq!(
        stats.warnings, 2,
        "one warning for the file, one for the directory"
    );
    let lossy: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM book_files WHERE abs_path LIKE '%' || chr(65533) || '%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lossy, 0);
}

/// Discovery does not follow symbolic links (same rule as the watcher).
#[cfg(unix)]
#[sqlx::test(migrations = "../../infra/migrations")]
async fn symlinks_are_not_followed(pool: sqlx::PgPool) {
    let f = Fixture::new(&pool).await;
    f.write_file("Series/Series - T01.cbz");
    let outside = f._dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("Other - T01.cbz"), b"zip").unwrap();
    std::os::unix::fs::symlink(&outside, f.root.join("Other")).unwrap();

    f.run().await;

    assert_eq!(f.book_titles().await, vec!["Series - T01"]);
}

/// Books stored under the lossy form of a path the walk skipped (non UTF-8 name) are kept;
/// rows elsewhere in the library stay eligible for deletion.
#[cfg(unix)]
#[test]
fn non_utf8_skips_protect_only_rows_under_the_skipped_path() {
    use super::NonUtf8Skips;
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let root = Path::new("/data/lib");
    let mut skips = NonUtf8Skips::default();
    skips.skip(&root.join(OsStr::from_bytes(b"Bad \xff Dir")), true);
    skips.skip(
        &root
            .join("Series")
            .join(OsStr::from_bytes(b"Series \xfe T02.cbz")),
        false,
    );

    let under_dir = "/data/lib/Bad \u{fffd} Dir/Inner - T01.cbz".to_string();
    let nested = "/data/lib/Bad \u{fffd} Dir/Sub/Inner - T02.cbz".to_string();
    let file = "/data/lib/Series/Series \u{fffd} T02.cbz".to_string();
    let sibling = "/data/lib/Bad \u{fffd} Dir2/Other.cbz".to_string();
    let gone = "/data/lib/Series/Series - T03.cbz".to_string();
    let seen_already = "/data/lib/Series/Series - T01.cbz".to_string();

    assert!(skips.protects(&under_dir));
    assert!(skips.protects(&nested));
    assert!(skips.protects(&file));
    assert!(
        !skips.protects(&sibling),
        "prefix match stops at the directory boundary"
    );
    assert!(!skips.protects(&gone));

    let existing = [&under_dir, &nested, &file, &sibling, &gone, &seen_already];
    let mut seen: HashMap<String, bool> = HashMap::from([(seen_already.clone(), true)]);
    let protected = skips.protect_existing(existing.into_iter(), &mut seen);

    assert_eq!(protected, 3);
    assert!(
        seen.contains_key(&under_dir) && seen.contains_key(&nested) && seen.contains_key(&file)
    );
    assert!(!seen.contains_key(&sibling) && !seen.contains_key(&gone));
}

/// With nothing skipped, no row is protected.
#[test]
fn empty_non_utf8_skips_protect_nothing() {
    let skips = super::NonUtf8Skips::default();
    assert!(!skips.protects("/data/lib/Series/Series - T01.cbz"));
}
