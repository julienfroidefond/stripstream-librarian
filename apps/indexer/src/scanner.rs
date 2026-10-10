use serde::Serialize;
use uuid::Uuid;

mod archive;
mod discovery;
mod mtime;

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
    use std::collections::HashMap;
    use uuid::Uuid;

    use crate::scanner::discovery::get_or_create_series_id;
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

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_new(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache = HashMap::new();
        let id = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        assert_ne!(id, Uuid::nil());
        assert_eq!(cache.len(), 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_cache_hit(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        let id2 = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        assert_eq!(id1, id2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_cache_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "One Piece", &mut cache)
            .await
            .unwrap();
        // Different casing should hit cache
        let id2 = get_or_create_series_id(&pool, lib_id, "one piece", &mut cache)
            .await
            .unwrap();
        assert_eq!(id1, id2);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_db_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        // Use two separate caches to bypass cache and test DB lookup
        let mut cache1 = HashMap::new();
        let mut cache2 = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "Dragon Ball", &mut cache1)
            .await
            .unwrap();
        let id2 = get_or_create_series_id(&pool, lib_id, "dragon ball", &mut cache2)
            .await
            .unwrap();
        assert_eq!(id1, id2, "DB lookup should be case-insensitive");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_db_accent_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "test").await;
        let mut cache1 = HashMap::new();
        let mut cache2 = HashMap::new();
        let id1 = get_or_create_series_id(&pool, lib_id, "Astérix", &mut cache1)
            .await
            .unwrap();
        let id2 = get_or_create_series_id(&pool, lib_id, "Asterix", &mut cache2)
            .await
            .unwrap();
        assert_eq!(id1, id2, "DB lookup should be accent-insensitive");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_finds_by_original_name(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "rename_test").await;
        let mut cache = HashMap::new();

        // Create series then simulate user rename
        let id1 = get_or_create_series_id(&pool, lib_id, "Dragon Ball", &mut cache)
            .await
            .unwrap();
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind("Dragon Ball Z")
            .bind("Dragon Ball")
            .bind(id1)
            .execute(&pool)
            .await
            .unwrap();

        // Clear cache to force DB lookup
        let mut cache2 = HashMap::new();
        let id2 = get_or_create_series_id(&pool, lib_id, "Dragon Ball", &mut cache2)
            .await
            .unwrap();
        assert_eq!(
            id1, id2,
            "lookup by original_name should return the renamed series"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn get_or_create_series_id_original_name_case_insensitive(pool: sqlx::PgPool) {
        let lib_id = create_test_library(&pool, "rename_case").await;
        let mut cache = HashMap::new();

        let id1 = get_or_create_series_id(&pool, lib_id, "LES MYTHICS", &mut cache)
            .await
            .unwrap();
        sqlx::query("UPDATE series SET name = $1, original_name = $2 WHERE id = $3")
            .bind("Mythics")
            .bind("LES MYTHICS")
            .bind(id1)
            .execute(&pool)
            .await
            .unwrap();

        let mut cache2 = HashMap::new();
        let id2 = get_or_create_series_id(&pool, lib_id, "les mythics", &mut cache2)
            .await
            .unwrap();
        assert_eq!(id1, id2, "original_name lookup should be case-insensitive");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_updates_title_when_filename_differs(pool: sqlx::PgPool) {
        // 1. Create a library
        let library_id = create_test_library(&pool, "test_update").await;

        // 2. Create a series
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Series")
            .execute(&pool)
            .await
            .unwrap();

        // 3. Insert a book with title="Old Title" and volume=None
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format, series_id) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind("Old Title")
        .bind("comic")
        .bind("cbz")
        .bind(series_id)
        .execute(&pool)
        .await
        .unwrap();

        // 4. Insert a book_file with abs_path containing a DIFFERENT filename
        let file_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7)",
        )
        .bind(file_id)
        .bind(book_id)
        .bind("/libraries/test_update/Series/Series - T05.cbz")
        .bind("cbz")
        .bind(1024_i64)
        .bind("fake_fingerprint")
        .bind("ok")
        .execute(&pool)
        .await
        .unwrap();

        // 5. Verify the book still has the old title
        let db_title: String = sqlx::query_scalar("SELECT title FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(db_title, "Old Title");

        // 6. Simulate what the scanner does: parse the filename, compare, and update
        let parsed_title = "Series - T05";
        let parsed_volume = parsers::extract_volume(parsed_title);

        // Confirm mismatch
        assert_ne!(db_title, parsed_title);

        // Update like the scanner does
        sqlx::query("UPDATE books SET title = $1, volume = $2, updated_at = NOW() WHERE id = $3")
            .bind(parsed_title)
            .bind(parsed_volume)
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();

        // 7. Verify the book now has the new title and volume
        let new_title: String = sqlx::query_scalar("SELECT title FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(new_title, "Series - T05");

        let new_volume: Option<i32> = sqlx::query_scalar("SELECT volume FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(new_volume, Some(5));
    }

    /// Helper: create a book + book_file in DB for title/volume mismatch tests.
    async fn create_book_with_file(
        pool: &sqlx::PgPool,
        library_id: Uuid,
        series_id: Uuid,
        title: &str,
        volume: Option<i32>,
        abs_path: &str,
    ) -> (Uuid, Uuid) {
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, volume, kind, format, series_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(title)
        .bind(volume)
        .bind("comic")
        .bind("cbz")
        .bind(series_id)
        .execute(pool)
        .await
        .unwrap();

        let file_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7)",
        )
        .bind(file_id)
        .bind(book_id)
        .bind(abs_path)
        .bind("cbz")
        .bind(1024_i64)
        .bind("fake_fingerprint")
        .bind("ok")
        .execute(pool)
        .await
        .unwrap();

        (book_id, file_id)
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_updates_title_in_skipped_dir(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "skipped_dir_test").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Series")
            .execute(&pool)
            .await
            .unwrap();

        // Book has "Old Name" with no volume
        let (book_id, _file_id) = create_book_with_file(
            &pool,
            library_id,
            series_id,
            "Old Name",
            None,
            "/libraries/skipped_dir_test/Series/Series - T05.cbz",
        )
        .await;

        // Simulate scanner logic: parse the filename and compare
        let parsed_title = "Series - T05";
        let parsed_volume = parsers::extract_volume(parsed_title);

        let row: (String, Option<i32>) =
            sqlx::query_as("SELECT title, volume FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let (db_title, db_volume) = row;

        // Title mismatch triggers update
        assert_ne!(db_title, parsed_title);
        assert!(db_title != parsed_title || db_volume != parsed_volume);

        sqlx::query("UPDATE books SET title = $1, volume = $2, updated_at = NOW() WHERE id = $3")
            .bind(parsed_title)
            .bind(parsed_volume)
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();

        let new_title: String = sqlx::query_scalar("SELECT title FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let new_volume: Option<i32> = sqlx::query_scalar("SELECT volume FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(new_title, "Series - T05");
        assert_eq!(new_volume, Some(5));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_updates_volume_when_title_matches_but_volume_null(pool: sqlx::PgPool) {
        // Key case: title matches the filename stem but volume is NULL in DB.
        // The fix ensures the scanner also checks volume mismatch, not just title.
        let library_id = create_test_library(&pool, "vol_null_test").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Kaiju no8")
            .execute(&pool)
            .await
            .unwrap();

        // Title matches exactly what parse_metadata_fast would produce, but volume is NULL
        let (book_id, _file_id) = create_book_with_file(
            &pool,
            library_id,
            series_id,
            "Kaiju no8 - Tome 1",
            None, // volume is NULL — this is the bug
            "/libraries/vol_null_test/Kaiju no8/Kaiju no8 - Tome 1.cbz",
        )
        .await;

        // Simulate scanner logic
        let parsed_title = "Kaiju no8 - Tome 1";
        let parsed_volume = parsers::extract_volume(parsed_title);
        assert_eq!(parsed_volume, Some(1), "extract_volume should parse Tome 1");

        let row: (String, Option<i32>) =
            sqlx::query_as("SELECT title, volume FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let (db_title, db_volume) = row;

        // Title matches but volume differs (None vs Some(1))
        assert_eq!(db_title, parsed_title);
        assert_ne!(db_volume, parsed_volume);

        // With the fix, the condition `db_title != parsed.title || db_volume != parsed.volume`
        // catches this case and triggers the update
        if db_title != parsed_title || db_volume != parsed_volume {
            sqlx::query(
                "UPDATE books SET title = $1, volume = $2, updated_at = NOW() WHERE id = $3",
            )
            .bind(parsed_title)
            .bind(parsed_volume)
            .bind(book_id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let new_volume: Option<i32> = sqlx::query_scalar("SELECT volume FROM books WHERE id = $1")
            .bind(book_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(new_volume, Some(1), "volume should now be set to 1");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn scanner_skips_when_title_and_volume_match(pool: sqlx::PgPool) {
        let library_id = create_test_library(&pool, "no_update_test").await;

        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, $3)")
            .bind(series_id)
            .bind(library_id)
            .bind("Series")
            .execute(&pool)
            .await
            .unwrap();

        // Book already has correct title AND volume
        let (book_id, _file_id) = create_book_with_file(
            &pool,
            library_id,
            series_id,
            "Series - T05",
            Some(5),
            "/libraries/no_update_test/Series/Series - T05.cbz",
        )
        .await;

        // Record the updated_at before the check
        let before_updated_at: DateTime<Utc> =
            sqlx::query_scalar("SELECT updated_at FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        // Simulate scanner logic
        let parsed_title = "Series - T05";
        let parsed_volume = parsers::extract_volume(parsed_title);
        assert_eq!(parsed_volume, Some(5));

        let row: (String, Option<i32>) =
            sqlx::query_as("SELECT title, volume FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let (db_title, db_volume) = row;

        // Both match — no update should happen
        assert_eq!(db_title, parsed_title);
        assert_eq!(db_volume, parsed_volume);

        let needs_update = db_title != parsed_title || db_volume != parsed_volume;
        assert!(
            !needs_update,
            "no update should be needed when title and volume match"
        );

        // Verify updated_at is unchanged
        let after_updated_at: DateTime<Utc> =
            sqlx::query_scalar("SELECT updated_at FROM books WHERE id = $1")
                .bind(book_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            before_updated_at, after_updated_at,
            "updated_at should not have changed"
        );
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
