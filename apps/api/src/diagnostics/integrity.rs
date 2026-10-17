//! Consistency counters that need no filesystem access.
//!
//! Cheap queries only: the exhaustive file-existence scan is an explicit,
//! on-demand probe (`/diagnostics/probe`) because it stats every book file.

use sqlx::Row;

use super::IntegrityDiagnostics;

pub(super) async fn integrity_diagnostics(
    pool: &sqlx::PgPool,
) -> Result<IntegrityDiagnostics, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*)
               FROM books b
              WHERE NOT EXISTS (SELECT 1 FROM book_files bf WHERE bf.book_id = b.id))
                AS books_without_files,
            (SELECT COUNT(*)
               FROM books
              WHERE page_count = 0)
                AS books_zero_pages,
            (SELECT COUNT(*)
               FROM series s
              WHERE NOT EXISTS (
                    SELECT 1 FROM external_metadata_links l WHERE l.series_id = s.id))
                AS series_without_metadata,
            (SELECT COUNT(*)
               FROM (
                    SELECT 1
                      FROM books
                     WHERE volume IS NOT NULL
                       AND series_id IS NOT NULL
                       AND volume_type = 'regular'
                     GROUP BY library_id, series_id, volume
                    HAVING COUNT(*) > 1
               ) duplicates)
                AS duplicate_volume_groups
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(IntegrityDiagnostics {
        books_without_files: row.get("books_without_files"),
        books_zero_pages: row.get("books_zero_pages"),
        series_without_metadata: row.get("series_without_metadata"),
        duplicate_volume_groups: row.get("duplicate_volume_groups"),
    })
}
