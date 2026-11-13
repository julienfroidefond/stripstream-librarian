//! The current file of a book: its most recently updated `book_files` row.
//!
//! [`current_book_file`] is the point lookup; [`current_book_file_join!`] is the same rule as
//! a SQL fragment for set-based queries over `books b` (the joined row is aliased `bf`).

use std::path::PathBuf;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use parsers::BookFormat;
use stripstream_core::paths::LibraryRoot;

use crate::error::ApiError;

/// `LEFT JOIN` of the current `book_files` row of `b.id`, aliased `bf`, projecting the
/// given comma-separated columns (kept explicit so that `bf` never shadows a `books`
/// column such as `format` or `updated_at`). Expands to a literal: use it in `concat!`
/// or as a `format!` argument.
macro_rules! current_book_file_join {
    ($columns:literal) => {
        concat!(
            "LEFT JOIN LATERAL (SELECT ",
            $columns,
            " FROM book_files WHERE book_id = b.id ORDER BY updated_at DESC LIMIT 1) bf ON TRUE"
        )
    };
}
pub(crate) use current_book_file_join;

/// The file a book is read, rendered and downloaded from.
#[derive(Debug, Clone)]
pub(crate) struct BookFile {
    /// Path as stored in the database (`/libraries/...`).
    pub db_path: String,
    /// The same path on this host's filesystem.
    pub local_path: PathBuf,
    /// Format recorded by the indexer — the only source of the input format.
    pub format: BookFormat,
}

/// The current file of `book_id`; `404` when the book has none.
pub(crate) async fn current_book_file(
    pool: &PgPool,
    library_root: &LibraryRoot,
    book_id: Uuid,
) -> Result<BookFile, ApiError> {
    // A one-row `b` so that the point lookup goes through the very same join.
    let row = sqlx::query(concat!(
        "SELECT bf.abs_path, bf.format FROM (SELECT $1::uuid AS id) b ",
        current_book_file_join!("abs_path, format"),
        " WHERE bf.abs_path IS NOT NULL"
    ))
    .bind(book_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("book file not found"))?;

    let db_path: String = row.get("abs_path");
    let format = row
        .get::<String, _>("format")
        .parse::<BookFormat>()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let local_path = PathBuf::from(library_root.resolve(&db_path)?.to_local());
    Ok(BookFile {
        db_path,
        local_path,
        format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_book(pool: &PgPool) -> Uuid {
        let library_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO libraries (id, name, root_path) VALUES ($1, 'files', '/libraries/files')",
        )
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, authors, locked_fields) VALUES ($1, $2, 'T01', 'comic', '{}'::text[], '{}'::jsonb)",
        )
        .bind(book_id)
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();
        book_id
    }

    async fn add_file(pool: &PgPool, book_id: Uuid, path: &str, format: &str, age_secs: i32) {
        sqlx::query(
            "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint, parse_status, updated_at) \
             VALUES ($1, $2, $3, $4, 1, NOW(), 'fp', 'ok', NOW() - $5::int * INTERVAL '1 second')",
        )
        .bind(Uuid::new_v4())
        .bind(book_id)
        .bind(format)
        .bind(path)
        .bind(age_secs)
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn current_book_file_picks_the_latest_row_and_resolves_it(pool: PgPool) {
        let book_id = seed_book(&pool).await;
        add_file(&pool, book_id, "/libraries/files/old.cbr", "cbr", 60).await;
        add_file(&pool, book_id, "/libraries/files/new.cbz", "cbz", 0).await;

        let file = current_book_file(&pool, &LibraryRoot::new("/mnt/books"), book_id)
            .await
            .unwrap();

        assert_eq!(file.db_path, "/libraries/files/new.cbz");
        assert_eq!(file.local_path, PathBuf::from("/mnt/books/files/new.cbz"));
        assert_eq!(file.format, BookFormat::Cbz);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn current_book_file_is_not_found_without_a_row(pool: PgPool) {
        let book_id = seed_book(&pool).await;

        let error = current_book_file(&pool, &LibraryRoot::new("/libraries"), book_id)
            .await
            .unwrap_err();

        assert_eq!(error.status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(error.message, "book file not found");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn current_book_file_join_exposes_the_latest_row_as_bf(pool: PgPool) {
        let book_id = seed_book(&pool).await;
        add_file(&pool, book_id, "/libraries/files/old.pdf", "pdf", 60).await;
        add_file(&pool, book_id, "/libraries/files/new.epub", "epub", 0).await;

        let format: String = sqlx::query_scalar(concat!(
            "SELECT bf.format FROM books b ",
            current_book_file_join!("format"),
            " WHERE b.id = $1"
        ))
        .bind(book_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(format, "epub");
    }
}
