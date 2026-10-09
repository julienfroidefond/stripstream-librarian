use anyhow::Result;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::utils;
use crate::AppState;

/// Check if any indexing job is currently active.
async fn has_active_jobs(pool: &sqlx::PgPool) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails'))",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}

/// Snapshot: set of book file paths found in a library.
type LibrarySnapshot = HashSet<String>;

/// Walk a library directory and collect all book file paths.
/// Walks sequentially to limit open file descriptors.
fn snapshot_library(root_path: &str) -> LibrarySnapshot {
    let mut files = HashSet::new();
    let walker = WalkDir::new(root_path)
        .follow_links(true)
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
    old_snapshot: &LibrarySnapshot,
    new_snapshot: &LibrarySnapshot,
) -> bool {
    // A newly appearing file must be indexed.
    if new_snapshot.iter().any(|path| !old_snapshot.contains(path)) {
        return false;
    }

    let removed_db_paths: Vec<String> = old_snapshot
        .iter()
        .filter(|path| !new_snapshot.contains(*path))
        .map(|path| utils::unmap_libraries_path(path))
        .collect();

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
        if has_active_jobs(&pool).await {
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
            .map(|row| {
                let id: Uuid = row.get("id");
                let root_path: String = row.get("root_path");
                let local_path = utils::remap_libraries_path(&root_path);
                (id, local_path)
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
            if has_active_jobs(&pool).await {
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
                        is_already_reconciled(&pool, old_snapshot, &new_snapshot).await
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
                    let job_exists = sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS(SELECT 1 FROM index_jobs WHERE library_id = $1 AND status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails'))",
                    )
                    .bind(library_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(true);

                    if !job_exists {
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
            is_already_reconciled(&pool, &old, &new).await,
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
            !is_already_reconciled(&pool, &old, &new).await,
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
            !is_already_reconciled(&pool, &old, &new).await,
            "a newly added file must trigger a rebuild"
        );
    }
}
