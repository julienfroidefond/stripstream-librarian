use anyhow::Result;
use tracing::{info, warn};
use uuid::Uuid;

/// Archive books + their files + their reading progress before deletion, in set-based statements.
pub async fn archive_books(
    pool: &sqlx::PgPool,
    book_ids: &[Uuid],
    file_ids: &[Uuid],
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO archived_books (id, library_id, series_id, series_name, kind, format, title,
                                    author, authors, volume, volume_type, language, page_count,
                                    thumbnail_path, locked_fields, summary, isbn, publish_date,
                                    created_at, updated_at)
        SELECT b.id, b.library_id, b.series_id, s.name, b.kind, b.format, b.title,
               b.author, b.authors, b.volume, b.volume_type, b.language, b.page_count,
               b.thumbnail_path, b.locked_fields, b.summary, b.isbn, b.publish_date,
               b.created_at, b.updated_at
        FROM books b
        LEFT JOIN series s ON s.id = b.series_id
        WHERE b.id = ANY($1)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(book_ids)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO archived_book_files (id, archived_book_id, format, abs_path, size_bytes, mtime, fingerprint, created_at)
        SELECT id, book_id, format, abs_path, size_bytes, mtime, fingerprint, created_at
        FROM book_files WHERE id = ANY($1)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(file_ids)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO archived_book_reading_progress (archived_book_id, user_id, status, current_page, last_read_at, updated_at)
        SELECT book_id, user_id, status, current_page, last_read_at, updated_at
        FROM book_reading_progress WHERE book_id = ANY($1)
        ON CONFLICT (archived_book_id, user_id) DO NOTHING
        "#,
    )
    .bind(book_ids)
    .execute(pool)
    .await?;

    Ok(())
}

/// Archive series that are about to lose all their books.
pub async fn archive_empty_series(pool: &sqlx::PgPool, series_ids: &[Uuid]) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO archived_series (id, library_id, name, description, authors, publishers, genres,
                                     start_year, total_volumes, status, locked_fields, original_name,
                                     book_author, book_language, cover_url, created_at, updated_at)
        SELECT id, library_id, name, description, authors, publishers, genres,
               start_year, total_volumes, status, locked_fields, original_name,
               book_author, book_language, cover_url, created_at, updated_at
        FROM series
        WHERE id = ANY($1)
          AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(series_ids)
    .execute(pool)
    .await?;

    // Preserve AniList link before the CASCADE delete wipes anilist_series_links
    sqlx::query(
        r#"
        UPDATE archived_series aseries
        SET anilist_id    = asl.anilist_id,
            anilist_title = asl.anilist_title,
            anilist_url   = asl.anilist_url
        FROM anilist_series_links asl
        WHERE asl.series_id = aseries.id
          AND aseries.id = ANY($1)
          AND asl.anilist_id IS NOT NULL
          AND aseries.anilist_id IS NULL
        "#,
    )
    .bind(series_ids)
    .execute(pool)
    .await?;

    Ok(())
}

/// Archive orphan series (no books, no metadata links, no available downloads).
pub async fn archive_orphan_series(pool: &sqlx::PgPool, library_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO archived_series (id, library_id, name, description, authors, publishers, genres,
                                     start_year, total_volumes, status, locked_fields, original_name,
                                     book_author, book_language, cover_url, created_at, updated_at)
        SELECT id, library_id, name, description, authors, publishers, genres,
               start_year, total_volumes, status, locked_fields, original_name,
               book_author, book_language, cover_url, created_at, updated_at
        FROM series
        WHERE library_id = $1
          AND NOT EXISTS (SELECT 1 FROM books WHERE series_id = series.id)
          AND NOT EXISTS (SELECT 1 FROM external_metadata_links WHERE series_id = series.id)
          AND NOT EXISTS (SELECT 1 FROM available_downloads WHERE series_id = series.id)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    // Preserve AniList link before the CASCADE delete wipes anilist_series_links
    sqlx::query(
        r#"
        UPDATE archived_series aseries
        SET anilist_id    = asl.anilist_id,
            anilist_title = asl.anilist_title,
            anilist_url   = asl.anilist_url
        FROM anilist_series_links asl
        JOIN series s ON s.id = asl.series_id
        WHERE asl.series_id = aseries.id
          AND s.library_id = $1
          AND asl.anilist_id IS NOT NULL
          AND aseries.anilist_id IS NULL
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// After a scan, restore reading progress and series metadata for re-discovered books/series.
/// Matches archived records by file path (books) or name+library (series).
pub async fn restore_archived_data(pool: &sqlx::PgPool, library_id: Uuid) -> Result<()> {
    // Restore reading progress by exact file path (same file re-discovered)
    let restored_by_path: i64 = sqlx::query_scalar(
        r#"
        WITH restored AS (
            INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
            SELECT b.id, abrp.user_id, abrp.status, abrp.current_page, abrp.last_read_at, abrp.updated_at
            FROM books b
            JOIN book_files bf ON bf.book_id = b.id
            JOIN archived_book_files abf ON abf.abs_path = bf.abs_path
            JOIN archived_book_reading_progress abrp ON abrp.archived_book_id = abf.archived_book_id
            WHERE b.library_id = $1
            ON CONFLICT (book_id, user_id) DO NOTHING
            RETURNING book_id
        )
        SELECT COUNT(*) FROM restored
        "#,
    )
    .bind(library_id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| {
        warn!(
            "[SCAN] Failed to restore reading progress by path for library {}: {}",
            library_id, e
        );
        0
    });

    // Fallback: match by volume for re-downloads whose filename/extension changed
    // (e.g. "Amulet - T2.cbr" replaced by "Amulet - 02.cbz"). A volume can have several
    // archived rows (multiple editions); keep the most recently archived one per user.
    let restored_by_volume: i64 = sqlx::query_scalar(
        r#"
        WITH restored AS (
            INSERT INTO book_reading_progress (book_id, user_id, status, current_page, last_read_at, updated_at)
            SELECT DISTINCT ON (b.id, abrp.user_id)
                b.id, abrp.user_id, abrp.status, abrp.current_page, abrp.last_read_at, abrp.updated_at
            FROM books b
            JOIN series s ON s.id = b.series_id
            JOIN archived_books ab
              ON ab.library_id = b.library_id
             AND (
                  ab.series_id = b.series_id
                  OR norm_text(ab.series_name) = norm_text(s.name)
                 )
             AND ab.volume = b.volume
             AND ab.volume_type = b.volume_type
             AND ab.kind = b.kind
            JOIN archived_book_reading_progress abrp ON abrp.archived_book_id = ab.id
            WHERE b.library_id = $1
              AND b.volume IS NOT NULL
              AND b.volume_type = 'regular'
            ORDER BY b.id, abrp.user_id, ab.archived_at DESC
            ON CONFLICT (book_id, user_id) DO NOTHING
            RETURNING book_id
        )
        SELECT COUNT(*) FROM restored
        "#,
    )
    .bind(library_id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| {
        warn!(
            "[SCAN] Failed to restore reading progress by volume for library {}: {}",
            library_id, e
        );
        0
    });

    let restored = restored_by_path + restored_by_volume;
    if restored > 0 {
        info!(
            "[SCAN] Restored reading progress for {} books in library {} ({} by path, {} by volume)",
            restored, library_id, restored_by_path, restored_by_volume
        );
    }

    // Restore series metadata for re-created series (only fill empty fields)
    sqlx::query(
        r#"
        UPDATE series s
        SET
            description     = COALESCE(s.description, aseries.description),
            authors         = CASE WHEN s.authors = '{}' THEN aseries.authors ELSE s.authors END,
            publishers      = CASE WHEN s.publishers = '{}' THEN aseries.publishers ELSE s.publishers END,
            genres          = CASE WHEN s.genres = '{}' THEN aseries.genres ELSE s.genres END,
            total_volumes   = COALESCE(s.total_volumes, aseries.total_volumes),
            status          = COALESCE(s.status, aseries.status),
            cover_url       = COALESCE(s.cover_url, aseries.cover_url),
            locked_fields   = CASE WHEN s.locked_fields = '{}' THEN aseries.locked_fields ELSE s.locked_fields END,
            updated_at      = NOW()
        FROM archived_series aseries
        WHERE s.library_id = $1
          AND s.library_id = aseries.library_id
          AND norm_text(s.name) = norm_text(aseries.name)
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    // Clean up archived books whose files are now active again
    sqlx::query(
        r#"
        DELETE FROM archived_books
        WHERE id IN (
            SELECT ab.id FROM archived_books ab
            JOIN archived_book_files abf ON abf.archived_book_id = ab.id
            JOIN book_files bf ON bf.abs_path = abf.abs_path
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Clean up archived books matched by volume, so resetting a book's progress afterwards is
    // not undone by the next scan re-running the volume fallback.
    sqlx::query(
        r#"
        DELETE FROM archived_books ab
        USING books b, series s
        WHERE s.id = b.series_id
          AND ab.library_id = b.library_id
          AND (
               ab.series_id = b.series_id
               OR norm_text(ab.series_name) = norm_text(s.name)
              )
          AND ab.volume = b.volume
          AND ab.volume_type = b.volume_type
          AND ab.kind = b.kind
          AND b.library_id = $1
          AND b.volume IS NOT NULL
          AND b.volume_type = 'regular'
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    // Clean up archived series whose series is now active again
    sqlx::query(
        r#"
        DELETE FROM archived_series aseries
        WHERE library_id = $1
          AND EXISTS (
              SELECT 1 FROM series s
              WHERE s.library_id = aseries.library_id
                AND norm_text(s.name) = norm_text(aseries.name)
          )
        "#,
    )
    .bind(library_id)
    .execute(pool)
    .await?;

    Ok(())
}
