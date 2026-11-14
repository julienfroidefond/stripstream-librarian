use serde::Serialize;
use uuid::Uuid;

mod archive;
mod discovery;
mod mtime;
mod reconcile;
mod writer;

pub use archive::restore_archived_data;
pub use discovery::scan_library_discovery;

#[derive(Serialize)]
pub struct JobStats {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub removed_files: usize,
    pub errors: usize,
    pub warnings: usize,
    pub new_series: usize,
    pub new_series_names: Vec<String>,
    pub new_book_titles: Vec<String>,
}

const BATCH_SIZE: usize = 100;

/// Symbolic links are never followed when walking a library — by the discovery scan, the
/// file count of a job, and the watcher alike, so the watcher never reports a file that the
/// scan would not index. Following links could loop, leave the library root (a path that
/// `LibraryRoot` cannot map back) or index the same book twice under two paths.
pub const FOLLOW_SYMLINKS: bool = false;

/// A file already present in the DB, with the book fields the scan loop compares against.
pub struct ExistingFile {
    pub file_id: Uuid,
    pub book_id: Uuid,
    pub fingerprint: String,
    pub title: String,
    pub volume: Option<i32>,
    pub volume_type: String,
    pub series_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use crate::scanner::mtime::should_skip_deletions;

    #[test]
    fn skip_deletions_when_root_not_accessible() {
        assert!(should_skip_deletions(false, 10, 10, 5, false));
    }

    #[test]
    fn skip_deletions_when_no_files_seen_but_existing() {
        // Volume probably not mounted — saw 0 files but DB has 50
        assert!(should_skip_deletions(true, 0, 50, 50, false));
    }

    #[test]
    fn skip_deletions_when_all_existing_are_stale_and_parents_missing() {
        // Every DB file is stale AND its directory is gone — unmounted volume, skip
        assert!(should_skip_deletions(true, 5, 10, 10, true));
    }

    #[test]
    fn allow_deletions_when_all_existing_are_stale_but_parents_present() {
        // All files replaced/renamed in place (e.g. re-download) — dirs still exist, delete
        assert!(!should_skip_deletions(true, 5, 10, 10, false));
    }

    #[test]
    fn allow_deletions_normal_case() {
        // Some stale files but most are still present — normal
        assert!(!should_skip_deletions(true, 45, 50, 5, false));
    }

    #[test]
    fn allow_deletions_no_stale() {
        assert!(!should_skip_deletions(true, 50, 50, 0, false));
    }

    #[test]
    fn allow_deletions_empty_db() {
        // No existing files in DB — nothing to delete anyway
        assert!(!should_skip_deletions(true, 10, 0, 0, false));
    }

    #[test]
    fn batch_structs_use_series_id() {
        use crate::batch::{BookInsert, BookUpdate};

        let series_id = Uuid::new_v4();
        let book = BookInsert {
            book_id: Uuid::new_v4(),
            library_id: Uuid::new_v4(),
            kind: "comic".to_string(),
            format: "cbz".to_string(),
            title: "Test".to_string(),
            series_id: Some(series_id),
            volume: Some(1),
            volume_type: "regular".to_string(),
            page_count: None,
            thumbnail_path: None,
            analysis_status: stripstream_core::analysis::AnalysisStatus::Discovered,
        };
        assert_eq!(book.series_id, Some(series_id));

        let update = BookUpdate {
            book_id: Uuid::new_v4(),
            title: "Test".to_string(),
            kind: "comic".to_string(),
            format: "cbz".to_string(),
            series_id: None,
            volume: None,
            volume_type: "regular".to_string(),
            page_count: None,
            clear_thumbnail: false,
            analysis_status: stripstream_core::analysis::AnalysisStatus::Discovered,
        };
        assert_eq!(update.series_id, None);
    }

    // ─── Integration tests (require PostgreSQL) ─────────────────────────

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

    // ─── Discovery end-to-end: both directory paths reconcile the same way ───

    struct ScanFixture {
        _dir: tempfile::TempDir,
        state: crate::AppState,
        root: std::path::PathBuf,
        library_id: Uuid,
        job_id: Uuid,
        book_id: Uuid,
        series_id: Uuid,
    }

    fn empty_stats() -> JobStats {
        JobStats {
            scanned_files: 0,
            indexed_files: 0,
            removed_files: 0,
            errors: 0,
            warnings: 0,
            new_series: 0,
            new_series_names: Vec::new(),
            new_book_titles: Vec::new(),
        }
    }

    /// One library `lib/Series/Series - T05.cbz` on disk, known in the DB as `db_title` /
    /// `db_volume`. `stale_fingerprint` makes the stored fingerprint differ from the file;
    /// `skip_dir` records a future mtime for `Series/` so discovery takes the unchanged-dir path.
    async fn scan_fixture(
        pool: sqlx::PgPool,
        db_title: &str,
        db_volume: Option<i32>,
        stale_fingerprint: bool,
        skip_dir: bool,
    ) -> ScanFixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let series_dir = root.join("Series");
        std::fs::create_dir_all(&series_dir).unwrap();
        let file_path = series_dir.join("Series - T05.cbz");
        std::fs::write(&file_path, b"not really a zip").unwrap();

        let state = crate::AppState {
            pool: pool.clone(),
            library_root: stripstream_core::paths::LibraryRoot::new(dir.path()),
        };
        let library_id = create_test_library(&pool, "lib").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Series')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, volume, kind, format, series_id, analysis_status) \
             VALUES ($1, $2, $3, $4, 'comic', 'cbz', $5, 'complete')",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(db_title)
        .bind(db_volume)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let meta = std::fs::metadata(&file_path).unwrap();
        let mtime: DateTime<Utc> = meta.modified().unwrap().into();
        let fingerprint = if stale_fingerprint {
            "stale_fingerprint".to_string()
        } else {
            crate::utils::compute_fingerprint(&file_path, meta.len(), &mtime).unwrap()
        };
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, '/libraries/lib/Series/Series - T05.cbz', 'cbz', $3, $4, $5, 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(meta.len() as i64)
        .bind(mtime)
        .bind(fingerprint)
        .execute(&pool)
        .await
        .unwrap();

        if skip_dir {
            sqlx::query(
                "INSERT INTO directory_mtimes (library_id, dir_path, mtime) \
                 VALUES ($1, '/libraries/lib/Series', NOW() + INTERVAL '1 day')",
            )
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'running')",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        ScanFixture {
            _dir: dir,
            state,
            root,
            library_id,
            job_id,
            book_id,
            series_id,
        }
    }

    async fn run_discovery(f: &ScanFixture) -> JobStats {
        let mut stats = empty_stats();
        let mut processed = 0;
        scan_library_discovery(
            &f.state,
            f.job_id,
            f.library_id,
            &f.root,
            &mut stats,
            &mut processed,
            1,
            false,
        )
        .await
        .unwrap();
        assert_eq!(processed, 1, "the known file counts toward progress");
        stats
    }

    async fn book_row(pool: &sqlx::PgPool, book_id: Uuid) -> (String, Option<i32>, String) {
        sqlx::query_as("SELECT title, volume, analysis_status FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn update_messages(pool: &sqlx::PgPool, job_id: Uuid) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT message FROM index_job_events WHERE job_id = $1 AND event_type = 'book_updated'",
        )
        .bind(job_id)
        .fetch_all(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn discovery_corrects_title_in_changed_dir(pool: sqlx::PgPool) {
        assert_title_corrected(pool, false).await;
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn discovery_corrects_title_in_unchanged_dir(pool: sqlx::PgPool) {
        assert_title_corrected(pool, true).await;
    }

    async fn assert_title_corrected(pool: sqlx::PgPool, skip_dir: bool) {
        {
            let f = scan_fixture(pool.clone(), "Old Title", None, false, skip_dir).await;

            let stats = run_discovery(&f).await;

            assert_eq!(
                book_row(&pool, f.book_id).await,
                ("Series - T05".to_string(), Some(5), "complete".to_string()),
                "skip_dir={skip_dir}: title and volume corrected, analysis kept"
            );
            assert_eq!(stats.indexed_files, 0, "skip_dir={skip_dir}");
            let messages = update_messages(&pool, f.job_id).await;
            assert_eq!(
                messages.len(),
                1,
                "skip_dir={skip_dir}: one book_updated event"
            );
            assert!(
                messages[0].starts_with("Title/volume updated"),
                "skip_dir={skip_dir}: {}",
                messages[0]
            );
            let series_id: Option<Uuid> =
                sqlx::query_scalar("SELECT series_id FROM books WHERE id = $1")
                    .bind(f.book_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(series_id, Some(f.series_id), "skip_dir={skip_dir}");
        }
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn discovery_leaves_matching_book_untouched(pool: sqlx::PgPool) {
        let f = scan_fixture(pool.clone(), "Series - T05", Some(5), false, true).await;
        let before: DateTime<Utc> =
            sqlx::query_scalar("SELECT updated_at FROM books WHERE id = $1")
                .bind(f.book_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        run_discovery(&f).await;

        let after: DateTime<Utc> = sqlx::query_scalar("SELECT updated_at FROM books WHERE id = $1")
            .bind(f.book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, after, "no write when nothing drifted");
        assert!(update_messages(&pool, f.job_id).await.is_empty());
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn discovery_reindexes_changed_file_in_changed_dir(pool: sqlx::PgPool) {
        assert_reindexed(pool, false).await;
    }

    /// `cp` over a file does not bump its directory mtime: the unchanged-dir path must still
    /// see the new fingerprint.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn discovery_reindexes_changed_file_in_unchanged_dir(pool: sqlx::PgPool) {
        assert_reindexed(pool, true).await;
    }

    async fn assert_reindexed(pool: sqlx::PgPool, skip_dir: bool) {
        {
            let f = scan_fixture(pool.clone(), "Series - T05", Some(5), true, skip_dir).await;

            let stats = run_discovery(&f).await;

            assert_eq!(stats.indexed_files, 1, "skip_dir={skip_dir}");
            assert_eq!(
                book_row(&pool, f.book_id).await.2,
                "discovered",
                "skip_dir={skip_dir}: book sent back to analysis"
            );
            let fingerprint: String =
                sqlx::query_scalar("SELECT fingerprint FROM book_files WHERE book_id = $1")
                    .bind(f.book_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_ne!(fingerprint, "stale_fingerprint", "skip_dir={skip_dir}");
            assert_eq!(
                update_messages(&pool, f.job_id).await,
                vec!["Book updated (fingerprint changed): Series - T05.cbz".to_string()],
                "skip_dir={skip_dir}"
            );
        }
    }

    // ─── Progress writes on incremental scans ───

    /// Runs discovery over `files` known, unchanged books in `lib/Series/` and returns how many
    /// times the job row was written (progress updates; each one also checks cancellation).
    async fn job_writes_during_discovery(pool: sqlx::PgPool, files: usize, skip_dir: bool) -> i64 {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let series_dir = root.join("Series");
        std::fs::create_dir_all(&series_dir).unwrap();

        let state = crate::AppState {
            pool: pool.clone(),
            library_root: stripstream_core::paths::LibraryRoot::new(dir.path()),
        };
        let library_id = create_test_library(&pool, "lib").await;
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Series')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        for volume in 1..=files {
            let file_name = format!("Series - T{volume:03}.cbz");
            let file_path = series_dir.join(&file_name);
            std::fs::write(&file_path, b"not really a zip").unwrap();
            let meta = std::fs::metadata(&file_path).unwrap();
            let mtime: DateTime<Utc> = meta.modified().unwrap().into();
            let fingerprint =
                crate::utils::compute_fingerprint(&file_path, meta.len(), &mtime).unwrap();
            let book_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO books (id, library_id, title, volume, kind, format, series_id, analysis_status) \
                 VALUES ($1, $2, $3, $4, 'comic', 'cbz', $5, 'complete')",
            )
            .bind(book_id)
            .bind(library_id)
            .bind(format!("Series - T{volume:03}"))
            .bind(volume as i32)
            .bind(series_id)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
                 VALUES ($1, $2, $3, 'cbz', $4, $5, $6, 'ok')",
            )
            .bind(Uuid::new_v4())
            .bind(book_id)
            .bind(format!("/libraries/lib/Series/{file_name}"))
            .bind(meta.len() as i64)
            .bind(mtime)
            .bind(fingerprint)
            .execute(&pool)
            .await
            .unwrap();
        }

        if skip_dir {
            sqlx::query(
                "INSERT INTO directory_mtimes (library_id, dir_path, mtime) \
                 VALUES ($1, '/libraries/lib/Series', NOW() + INTERVAL '1 day')",
            )
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'running')",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::raw_sql(
            "CREATE TABLE job_writes (n INT NOT NULL); \
             CREATE FUNCTION count_job_write() RETURNS trigger AS $$ \
             BEGIN INSERT INTO job_writes VALUES (1); RETURN NEW; END $$ LANGUAGE plpgsql; \
             CREATE TRIGGER count_job_write AFTER UPDATE ON index_jobs \
             FOR EACH ROW EXECUTE FUNCTION count_job_write();",
        )
        .execute(&pool)
        .await
        .unwrap();

        let mut stats = empty_stats();
        let mut processed = 0;
        scan_library_discovery(
            &state,
            job_id,
            library_id,
            &root,
            &mut stats,
            &mut processed,
            files,
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            processed as usize, files,
            "every known file counts toward progress"
        );
        assert_eq!(stats.indexed_files, 0, "nothing changed on disk");

        sqlx::query_scalar("SELECT COUNT(*) FROM job_writes")
            .fetch_one(&pool)
            .await
            .unwrap()
    }

    /// An incremental scan over an unchanged directory must not write progress (and check
    /// cancellation) every 10 files: only the elapsed-time trigger applies there.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn unchanged_dir_files_do_not_trigger_count_based_progress(pool: sqlx::PgPool) {
        let writes = job_writes_during_discovery(pool, 50, true).await;
        assert!(
            writes <= 1,
            "50 files of an unchanged dir wrote the job row {writes} times"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn changed_dir_files_report_progress_every_10_files(pool: sqlx::PgPool) {
        let writes = job_writes_during_discovery(pool, 50, false).await;
        assert!(
            writes >= 5,
            "expected a progress write per 10 files, got {writes}"
        );
    }

    // ─── New series notifications ───

    /// A folder spelled differently from an existing series resolves to it through
    /// `series_identity`: it must not be reported as a new series.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn new_series_counts_only_series_actually_created(pool: sqlx::PgPool) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        for (folder, file) in [
            ("one piece", "one piece - T01.cbz"),
            ("Naruto", "Naruto - T01.cbz"),
            ("naruto", "naruto - T02.cbz"),
        ] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            std::fs::write(root.join(folder).join(file), b"not really a zip").unwrap();
        }
        let state = crate::AppState {
            pool: pool.clone(),
            library_root: stripstream_core::paths::LibraryRoot::new(dir.path()),
        };
        let library_id = create_test_library(&pool, "lib").await;
        let one_piece = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'One Piece')")
            .bind(one_piece)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'running')",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let mut stats = empty_stats();
        let mut processed = 0;
        scan_library_discovery(
            &state,
            job_id,
            library_id,
            &root,
            &mut stats,
            &mut processed,
            3,
            false,
        )
        .await
        .unwrap();

        assert_eq!(stats.indexed_files, 3);
        assert_eq!(stats.new_series, 1, "{:?}", stats.new_series_names);
        assert_eq!(stats.new_series_names.len(), 1);
        assert!(
            stats.new_series_names[0].eq_ignore_ascii_case("naruto"),
            "only Naruto is new: {:?}",
            stats.new_series_names
        );
        let one_piece_books: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE series_id = $1")
                .bind(one_piece)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(one_piece_books, 1, "'one piece' joins 'One Piece'");
        let series: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE library_id = $1")
            .bind(library_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(series, 2, "One Piece + Naruto");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn orphan_series_cleaned_up_after_book_deletion(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "orphan_test").await;

        // Create three series
        let series_with_books = Uuid::new_v4();
        let series_empty_no_links = Uuid::new_v4();
        let series_empty_with_metadata = Uuid::new_v4();
        for (id, name) in [
            (series_with_books, "Has Books"),
            (series_empty_no_links, "Empty No Links"),
            (series_empty_with_metadata, "Empty With Metadata"),
        ] {
            sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(library_id)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }

        // First series has a book
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format, series_id) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind("Book 1")
        .bind("comic")
        .bind("cbz")
        .bind(series_with_books)
        .execute(&pool)
        .await
        .unwrap();

        // Third series has a metadata link (added from Discovery)
        sqlx::query(
            "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id, external_url) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(library_id)
        .bind(series_empty_with_metadata)
        .bind("senscritique")
        .bind("12345")
        .bind("https://www.senscritique.com/serie/12345")
        .execute(&pool)
        .await
        .unwrap();

        // Delete orphan series (no books, no metadata, no downloads)
        let deleted = sqlx::query_scalar::<_, i32>(
            "DELETE FROM series WHERE library_id = $1 \
             AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id) \
             AND NOT EXISTS (SELECT 1 FROM external_metadata_links WHERE series_id = series.id) \
             AND NOT EXISTS (SELECT 1 FROM available_downloads WHERE series_id = series.id) \
             RETURNING 1",
        )
        .bind(library_id)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            deleted.len(),
            1,
            "should delete only the truly orphan series"
        );

        // Verify: series_with_books and series_empty_with_metadata still exist
        let remaining: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM series WHERE library_id = $1 ORDER BY name")
                .bind(library_id)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&series_with_books));
        assert!(remaining.contains(&series_empty_with_metadata));
        assert!(!remaining.contains(&series_empty_no_links));
    }

    /// Series that lost ALL books due to stale file deletion should be removed
    /// even if they have metadata links. This simulates a directory rename/delete.
    /// Discovery-created series (never had books deleted) are preserved.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn stale_deletion_removes_series_that_lost_all_books(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "stale_series_test").await;

        // Series A: had books, will lose them (directory deleted) — has metadata link
        let series_deleted_dir = Uuid::new_v4();
        // Series B: discovery series, never had books — has metadata link
        let series_discovery = Uuid::new_v4();
        // Series C: has books, keeps them
        let series_kept = Uuid::new_v4();

        for (id, name) in [
            (series_deleted_dir, "Deleted Dir Series"),
            (series_discovery, "Discovery Series"),
            (series_kept, "Kept Series"),
        ] {
            sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(library_id)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }

        // Both series_deleted_dir and series_discovery have metadata links
        for sid in [series_deleted_dir, series_discovery] {
            sqlx::query(
                "INSERT INTO external_metadata_links (library_id, series_id, provider, external_id) \
                 VALUES ($1, $2, 'senscritique', $3)",
            )
            .bind(library_id).bind(sid).bind(Uuid::new_v4().to_string())
            .execute(&pool).await.unwrap();
        }

        // series_deleted_dir has a book (will be deleted as stale)
        let stale_book_id = Uuid::new_v4();
        sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id) VALUES ($1, $2, 'Book 1', 'comic', 'cbz', $3)")
            .bind(stale_book_id).bind(library_id).bind(series_deleted_dir).execute(&pool).await.unwrap();

        // series_kept also has a book (stays)
        let kept_book_id = Uuid::new_v4();
        sqlx::query("INSERT INTO books (id, library_id, title, kind, format, series_id) VALUES ($1, $2, 'Book 2', 'comic', 'cbz', $3)")
            .bind(kept_book_id).bind(library_id).bind(series_kept).execute(&pool).await.unwrap();

        // Simulate stale deletion: delete the book from series_deleted_dir
        sqlx::query("DELETE FROM books WHERE id = $1")
            .bind(stale_book_id)
            .execute(&pool)
            .await
            .unwrap();

        // Now the stale series cleanup: series that just lost all books
        let affected_series_ids = vec![series_deleted_dir];
        let deleted_series = sqlx::query_scalar::<_, Uuid>(
            "DELETE FROM series WHERE id = ANY($1) \
             AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id) \
             RETURNING id",
        )
        .bind(&affected_series_ids)
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            deleted_series.len(),
            1,
            "series that lost all books should be deleted"
        );
        assert_eq!(deleted_series[0], series_deleted_dir);

        // Verify discovery series is NOT affected (not in affected_series_ids)
        let discovery_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
                .bind(series_discovery)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(discovery_exists, "discovery series should be preserved");

        // Verify kept series still exists
        let kept_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM series WHERE id = $1)")
                .bind(series_kept)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            kept_exists,
            "series with remaining books should be preserved"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn restore_reading_progress_by_volume_after_extension_change(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "restore_vol").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Amulet')")
            .bind(series_id)
            .bind(library_id)
            .execute(&pool)
            .await
            .unwrap();

        // Given: a re-downloaded book for volume 2 with a new filename/extension (.cbr -> .cbz)
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, volume, volume_type, kind, format, series_id) \
             VALUES ($1, $2, 'Amulet - 02', 2, 'regular', 'comic', 'cbz', $3)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, '/libraries/restore_vol/Amulet/Amulet - 02.cbz', 'cbz', 1024, NOW(), 'fp_new', 'ok')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .execute(&pool)
        .await
        .unwrap();

        // And: the archived old file for the same volume with its reading progress
        let archived_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO archived_books (id, library_id, series_id, series_name, kind, format, title, volume, volume_type) \
             VALUES ($1, $2, $3, 'Amulet', 'comic', 'cbr', 'Amulet - T2', 2, 'regular')",
        )
        .bind(archived_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, username) VALUES ($1, $2)")
            .bind(user_id)
            .bind(format!("u_{}", user_id))
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO archived_book_reading_progress (archived_book_id, user_id, status, current_page) \
             VALUES ($1, $2, 'read', 42)",
        )
        .bind(archived_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        // When: archived data is restored
        restore_archived_data(&pool, library_id).await.unwrap();

        // Then: progress is restored despite the path/extension change
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(status.as_deref(), Some("read"));

        let page: Option<i32> = sqlx::query_scalar(
            "SELECT current_page FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(page, Some(42));

        // And: the matched archived book is cleaned up so the fallback stays idempotent
        let archived_left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM archived_books WHERE id = $1")
                .bind(archived_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(archived_left, 0, "matched archived book must be cleaned up");

        // And: resetting progress (mark unread deletes the row) survives a later re-scan
        sqlx::query("DELETE FROM book_reading_progress WHERE book_id = $1 AND user_id = $2")
            .bind(book_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        restore_archived_data(&pool, library_id).await.unwrap();

        let status_after_reset: Option<String> = sqlx::query_scalar(
            "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(
            status_after_reset, None,
            "reset progress must not be re-injected"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn restore_reading_progress_by_volume_when_series_renamed(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "restore_renamed").await;

        // Given: a series renamed since the book was archived — same id, different name
        let series_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Amulet (nouvelle edition)')",
        )
        .bind(series_id)
        .bind(library_id)
        .execute(&pool)
        .await
        .unwrap();

        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, volume, volume_type, kind, format, series_id) \
             VALUES ($1, $2, 'Amulet - 02', 2, 'regular', 'comic', 'cbz', $3)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        // And: the archived row still carries the old series name
        let archived_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO archived_books (id, library_id, series_id, series_name, kind, format, title, volume, volume_type) \
             VALUES ($1, $2, $3, 'Amulet', 'comic', 'cbr', 'Amulet - T2', 2, 'regular')",
        )
        .bind(archived_id)
        .bind(library_id)
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, username) VALUES ($1, $2)")
            .bind(user_id)
            .bind(format!("u_{}", user_id))
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO archived_book_reading_progress (archived_book_id, user_id, status, current_page) \
             VALUES ($1, $2, 'read', 42)",
        )
        .bind(archived_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

        // When: archived data is restored
        restore_archived_data(&pool, library_id).await.unwrap();

        // Then: progress is restored via series_id despite the name mismatch
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM book_reading_progress WHERE book_id = $1 AND user_id = $2",
        )
        .bind(book_id)
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(status.as_deref(), Some("read"));

        let archived_left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM archived_books WHERE id = $1")
                .bind(archived_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(archived_left, 0, "matched archived book must be cleaned up");
    }
}
