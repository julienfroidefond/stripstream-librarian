use anyhow::Result;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use stripstream_jobs::{job_in_flight, JobScope, JobTypes};

use crate::AppState;

/// True when any job holds a slot, whatever its type or library.
///
/// A failed lookup lets the poll proceed: the snapshot comparison is cheap and
/// the next poll retries. The watcher never aborts on a transient database
/// error just because it could not ask the queue.
async fn any_job_in_flight(pool: &sqlx::PgPool) -> bool {
    match job_in_flight(pool, JobScope::Any, JobTypes::Any).await {
        Ok(job) => job.is_some(),
        Err(error) => {
            warn!("[WATCHER] Cannot check in-flight jobs, polling anyway: {error}");
            false
        }
    }
}

/// True when `library_id` already has a job in flight, whatever its type.
///
/// A failed lookup is treated as busy so the watcher never queues a duplicate
/// rebuild; the next poll retries.
async fn library_is_busy(pool: &sqlx::PgPool, library_id: Uuid) -> bool {
    match job_in_flight(pool, JobScope::Library(library_id), JobTypes::Any).await {
        Ok(job) => job.is_some(),
        Err(error) => {
            warn!("[WATCHER] Cannot check jobs for library {library_id}, assuming busy: {error}");
            true
        }
    }
}

/// Snapshot: set of book file paths found in a library.
type LibrarySnapshot = HashSet<String>;

/// Walk a library directory and collect all book file paths.
/// Walks sequentially to limit open file descriptors. Symbolic links follow the discovery
/// rule ([`crate::scanner::FOLLOW_SYMLINKS`]): a symlinked file the scan ignores must not
/// trigger a rebuild.
fn snapshot_library(root_path: &str) -> LibrarySnapshot {
    let mut files = HashSet::new();
    let walker = WalkDir::new(root_path)
        .follow_links(crate::scanner::FOLLOW_SYMLINKS)
        .max_open(10)
        .into_iter()
        .filter_map(|e| e.ok());

    for entry in walker {
        if entry.file_type().is_file() && parsers::detect_format(entry.path()).is_some() {
            files.insert(entry.path().to_string_lossy().to_string());
        }
    }
    files
}

/// Returns `true` when every difference between `old_snapshot` and `new_snapshot`
/// is a deletion already reflected in the database (e.g. a book or series removed
/// through the API). In that case the index is already consistent and a rebuild
/// would be redundant.
///
/// New files, or removals of files still tracked in `book_files`, always require
/// a rebuild so the scanner can index or remove them.
async fn is_already_reconciled(
    pool: &sqlx::PgPool,
    library_root: &stripstream_core::paths::LibraryRoot,
    old_snapshot: &LibrarySnapshot,
    new_snapshot: &LibrarySnapshot,
) -> bool {
    // A newly appearing file must be indexed.
    if new_snapshot.iter().any(|path| !old_snapshot.contains(path)) {
        return false;
    }

    let mut removed_db_paths = Vec::new();
    for path in old_snapshot
        .iter()
        .filter(|path| !new_snapshot.contains(*path))
    {
        match library_root.resolve(path) {
            Ok(path) => removed_db_paths.push(path.to_db().to_string()),
            Err(error) => {
                warn!("[WATCHER] Cannot map removed path {path} back to the database: {error}");
                return false;
            }
        }
    }

    if removed_db_paths.is_empty() {
        return true;
    }

    // If any removed file is still tracked in the DB, a rebuild is required so the
    // scanner removes the stale record. On query error, fall back to rebuilding.
    let still_tracked = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM book_files WHERE abs_path = ANY($1))",
    )
    .bind(&removed_db_paths)
    .fetch_one(pool)
    .await
    .unwrap_or(true);

    !still_tracked
}

pub async fn run_file_watcher(state: AppState) -> Result<()> {
    let poll_interval = Duration::from_secs(30);
    let pool = state.pool.clone();

    // Stored snapshots per library — used to detect additions/removals between polls
    let mut snapshots: HashMap<Uuid, LibrarySnapshot> = HashMap::new();
    // Track which libraries we're watching (to detect config changes)
    let mut watched_libraries: HashMap<Uuid, String> = HashMap::new();

    loop {
        tokio::time::sleep(poll_interval).await;

        // Skip if any job is active — avoid competing for file descriptors
        if any_job_in_flight(&pool).await {
            debug!(target: "watcher", "[WATCHER] Skipping poll — job active");
            continue;
        }

        // Fetch enabled libraries with watcher
        let rows = match sqlx::query(
            "SELECT id, root_path FROM libraries WHERE watcher_enabled = TRUE AND enabled = TRUE",
        )
        .fetch_all(&pool)
        .await
        {
            Ok(rows) => rows,
            Err(err) => {
                error!("[WATCHER] Failed to fetch libraries: {}", err);
                continue;
            }
        };

        let current_libraries: HashMap<Uuid, String> = rows
            .into_iter()
            .filter_map(|row| {
                let id: Uuid = row.get("id");
                let root_path: String = row.get("root_path");
                match state.library_root.resolve(&root_path) {
                    Ok(path) => Some((id, path.to_local().to_string())),
                    Err(error) => {
                        error!("[WATCHER] Invalid root_path for library {id}: {error}");
                        None
                    }
                }
            })
            .collect();

        // If library config changed, reset snapshots for removed/changed libraries
        if current_libraries != watched_libraries {
            let removed: Vec<Uuid> = watched_libraries
                .keys()
                .filter(|id| !current_libraries.contains_key(id))
                .copied()
                .collect();
            for id in removed {
                snapshots.remove(&id);
            }
            if !current_libraries.is_empty() {
                info!(
                    "[WATCHER] Watching {} libraries (lightweight poll, {}s interval)",
                    current_libraries.len(),
                    poll_interval.as_secs()
                );
            } else {
                info!("[WATCHER] No libraries to watch");
            }
            watched_libraries = current_libraries.clone();
        }

        // Poll each library sequentially to limit concurrent file descriptor usage
        for (library_id, root_path) in &current_libraries {
            if !Path::new(root_path).is_dir() {
                warn!(
                    "[WATCHER] Library {} path not accessible: {}",
                    library_id, root_path
                );
                continue;
            }

            // Re-check between libraries in case a job was created
            if any_job_in_flight(&pool).await {
                debug!(target: "watcher", "[WATCHER] Job became active during poll, stopping");
                break;
            }

            let root_owned = root_path.clone();
            let new_snapshot = tokio::task::spawn_blocking(move || snapshot_library(&root_owned))
                .await
                .unwrap_or_default();

            let changed = match snapshots.get(library_id) {
                Some(old_snapshot) => *old_snapshot != new_snapshot,
                None => {
                    // First scan — store baseline, don't trigger a job
                    debug!(
                        target: "watcher",
                        "[WATCHER] Initial snapshot for library {}: {} files",
                        library_id,
                        new_snapshot.len()
                    );
                    snapshots.insert(*library_id, new_snapshot);
                    continue;
                }
            };

            if changed {
                info!(
                    "[WATCHER] Changes detected in library {} ({})",
                    library_id, root_path
                );

                let already_reconciled = match snapshots.get(library_id) {
                    Some(old_snapshot) => {
                        is_already_reconciled(
                            &pool,
                            &state.library_root,
                            old_snapshot,
                            &new_snapshot,
                        )
                        .await
                    }
                    None => false,
                };

                if already_reconciled {
                    info!(
                        "[WATCHER] Changes in library {} already reflected in the database, skipping rebuild",
                        library_id
                    );
                } else {
                    // Check if a job already exists for this library
                    if !library_is_busy(&pool, *library_id).await {
                        let job_id = Uuid::new_v4();
                        match sqlx::query(
                            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'rebuild', 'pending')",
                        )
                        .bind(job_id)
                        .bind(library_id)
                        .execute(&pool)
                        .await
                        {
                            Ok(_) => info!(
                                "[WATCHER] Created rebuild job {} for library {}",
                                job_id, library_id
                            ),
                            Err(err) => error!("[WATCHER] Failed to create job: {}", err),
                        }
                    } else {
                        debug!(target: "watcher", "[WATCHER] Job already active for library {}, skipping", library_id);
                    }
                }
            }

            // Update snapshot
            snapshots.insert(*library_id, new_snapshot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_library(pool: &sqlx::PgPool, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(name)
            .bind(format!("/libraries/{name}"))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn create_book_with_file(pool: &sqlx::PgPool, lib_id: Uuid, abs_path: &str) -> Uuid {
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format) \
             VALUES ($1, $2, 'Book', 'comic', 'cbz')",
        )
        .bind(book_id)
        .bind(lib_id)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, 'cbz', $3, 1024, NOW(), 'fp', 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(abs_path)
        .execute(pool)
        .await
        .unwrap();
        book_id
    }

    fn snapshot(paths: &[&str]) -> LibrarySnapshot {
        paths.iter().map(|p| p.to_string()).collect()
    }

    async fn insert_watch_job(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        job_type: &str,
        status: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(library_id)
        .bind(job_type)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn terminate(pool: &sqlx::PgPool, job_id: Uuid) {
        sqlx::query("UPDATE index_jobs SET status = 'success' WHERE id = $1")
            .bind(job_id)
            .execute(pool)
            .await
            .unwrap();
    }

    /// The watcher's "any job holds a slot" rule comes from the shared predicate
    /// and covers every in-flight phase, whatever the job type or library.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn any_job_in_flight_covers_every_phase(pool: sqlx::PgPool) {
        assert!(!any_job_in_flight(&pool).await);

        let lib_id = create_test_library(&pool, "watch_any").await;
        for status in [
            "pending",
            "running",
            "extracting_pages",
            "generating_thumbnails",
        ] {
            let job_id = insert_watch_job(&pool, lib_id, "metadata_batch", status).await;
            assert!(any_job_in_flight(&pool).await, "status={status}");
            terminate(&pool, job_id).await;
        }
        assert!(!any_job_in_flight(&pool).await);
    }

    /// A busy library blocks only its own rebuild, and a terminal job frees it.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn library_is_busy_is_scoped_and_ignores_terminal_jobs(pool: sqlx::PgPool) {
        let first = create_test_library(&pool, "watch_busy").await;
        let second = create_test_library(&pool, "watch_idle").await;
        let job_id = insert_watch_job(&pool, first, "metadata_batch", "running").await;

        assert!(library_is_busy(&pool, first).await);
        assert!(!library_is_busy(&pool, second).await);

        terminate(&pool, job_id).await;
        assert!(!library_is_busy(&pool, first).await);
    }

    fn library_root() -> stripstream_core::paths::LibraryRoot {
        stripstream_core::paths::LibraryRoot::new("/libraries")
    }

    /// A deletion already applied through the API (DB row gone) needs no rebuild.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn api_deletion_is_already_reconciled(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "watch_api").await;
        let book_id = create_book_with_file(&pool, lib_id, "/libraries/watch_api/a.cbz").await;

        // Simulate the API deleting the book and its file row.
        sqlx::query("DELETE FROM books WHERE id = $1")
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();

        let old = snapshot(&["/libraries/watch_api/a.cbz"]);
        let new = snapshot(&[]);
        assert!(
            is_already_reconciled(&pool, &library_root(), &old, &new).await,
            "API deletion is already reflected in the DB"
        );
    }

    /// A file removed outside the app must still trigger a rebuild.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn external_deletion_requires_rebuild(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "watch_ext").await;
        create_book_with_file(&pool, lib_id, "/libraries/watch_ext/a.cbz").await;

        let old = snapshot(&["/libraries/watch_ext/a.cbz"]);
        let new = snapshot(&[]);
        assert!(
            !is_already_reconciled(&pool, &library_root(), &old, &new).await,
            "external deletion must trigger a rebuild"
        );
    }

    /// A new file always requires a rebuild, even when only deletions match.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn addition_requires_rebuild(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "watch_add").await;
        let book_id = create_book_with_file(&pool, lib_id, "/libraries/watch_add/a.cbz").await;
        sqlx::query("DELETE FROM books WHERE id = $1")
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();

        let old = snapshot(&["/libraries/watch_add/a.cbz"]);
        let new = snapshot(&["/libraries/watch_add/b.cbz"]);
        assert!(
            !is_already_reconciled(&pool, &library_root(), &old, &new).await,
            "a newly added file must trigger a rebuild"
        );
    }

    /// The watcher must see exactly what discovery indexes: symlinked files and directories
    /// are ignored by both (see `scanner::FOLLOW_SYMLINKS`).
    #[cfg(unix)]
    #[test]
    fn snapshot_ignores_symlinks_like_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(outside.join("Linked")).unwrap();
        std::fs::write(outside.join("Linked/Linked - T01.cbz"), b"zip").unwrap();
        std::fs::write(outside.join("Single.cbz"), b"zip").unwrap();

        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("Series")).unwrap();
        let real = root.join("Series/Series - T01.cbz");
        std::fs::write(&real, b"zip").unwrap();
        std::os::unix::fs::symlink(outside.join("Linked"), root.join("Linked")).unwrap();
        std::os::unix::fs::symlink(outside.join("Single.cbz"), root.join("Series/Single.cbz"))
            .unwrap();

        let files = snapshot_library(root.to_str().unwrap());

        assert_eq!(files, snapshot(&[real.to_str().unwrap()]));
    }
}
