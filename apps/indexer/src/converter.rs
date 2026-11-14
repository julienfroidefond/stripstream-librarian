use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Row, Transaction};
use tracing::{info, warn};
use uuid::Uuid;

use parsers::BookFormat;
use stripstream_jobs::{insert_entity_event, set_current_file};

use crate::{utils, AppState};

/// Convert `cbr_path`, or adopt the CBZ a previous run already produced.
///
/// The conversion publishes the CBZ with an atomic rename before the database is updated and
/// the CBR removed: a crash in between leaves both files. When the existing CBZ is exactly the
/// conversion of the CBR it is reused; any other file of that name is never overwritten.
fn convert_or_adopt(cbr_path: &Path) -> Result<PathBuf> {
    let cbz_path = cbr_path.with_extension("cbz");
    if cbz_path.exists() {
        if parsers::cbz_is_conversion_of(cbr_path, &cbz_path)? {
            info!(
                "[CONVERTER] Resuming: {} is already the conversion of {}",
                cbz_path.display(),
                cbr_path.display()
            );
            return Ok(cbz_path);
        }
        return Err(anyhow::anyhow!(
            "CBZ file already exists and is not a conversion of {}: {}",
            cbr_path.display(),
            cbz_path.display()
        ));
    }
    parsers::convert_cbr_to_cbz(cbr_path)
}

/// Execute a `cbr_to_cbz` job for the given `book_id`.
///
/// Flow:
/// 1. Read book file info from DB
/// 2. Resolve physical path
/// 3. Convert CBR → CBZ via `parsers::convert_cbr_to_cbz` (in `spawn_blocking`), or adopt
///    the complete CBZ a crashed run left next to the CBR
/// 4. Update `book_files` (path, format, size, mtime, fingerprint) and `books.format` in one
///    transaction
/// 5. Delete the original CBR (failure here does not fail the job)
///
/// The job's terminal status is written by the worker from the job outcome.
pub async fn convert_book(state: &AppState, job_id: Uuid, book_id: Uuid) -> Result<()> {
    info!(
        "[CONVERTER] Starting CBR→CBZ conversion for book {} (job {})",
        book_id, job_id
    );

    match convert_book_inner(state, job_id, book_id).await {
        Ok(book_name) => {
            insert_entity_event(
                &state.pool,
                job_id,
                "converted",
                "info",
                Some(&book_name),
                None,
                None,
            )
            .await;
            Ok(())
        }
        Err(e) => {
            insert_entity_event(
                &state.pool,
                job_id,
                "error",
                "error",
                None,
                Some(&e.to_string()),
                None,
            )
            .await;
            Err(e)
        }
    }
}

async fn convert_book_inner(state: &AppState, job_id: Uuid, book_id: Uuid) -> Result<String> {
    // Fetch current file info
    let row = sqlx::query(
        r#"
        SELECT bf.id as file_id, bf.abs_path, bf.format
        FROM book_files bf
        WHERE bf.book_id = $1
        ORDER BY bf.updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(book_id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| anyhow::anyhow!("no book file found for book {}", book_id))?;

    let file_id: Uuid = row.get("file_id");
    let abs_path: String = row.get("abs_path");
    let format: String = row.get("format");

    if format != "cbr" {
        return Err(anyhow::anyhow!(
            "book {} is not CBR (format={}), skipping conversion",
            book_id,
            format
        ));
    }

    let physical_path = state
        .library_root
        .resolve(&abs_path)?
        .to_local()
        .to_string();
    let cbr_path = std::path::Path::new(&physical_path);

    let book_name = cbr_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| abs_path.clone());

    info!("[CONVERTER] Converting {} → CBZ", cbr_path.display());

    // Update job status to running (already set by claim_next_job, this updates current_file)
    set_current_file(&state.pool, job_id, &abs_path).await?;

    // Opening and repacking the archive is blocking, CPU and I/O bound work.
    let source = cbr_path.to_path_buf();
    let cbz_path = tokio::task::spawn_blocking(move || convert_or_adopt(&source))
        .await
        .context("CBR conversion task failed")??;

    info!("[CONVERTER] CBZ ready at {}", cbz_path.display());

    // Remap physical path back to /libraries/ canonical form
    let new_abs_path = state
        .library_root
        .resolve(&cbz_path.to_string_lossy())?
        .to_db()
        .to_string();

    // The file row describes the new file: without its own size/mtime/fingerprint the next
    // scan would see a "changed" book and send it back to analysis.
    let metadata = std::fs::metadata(&cbz_path)
        .with_context(|| format!("cannot stat {}", cbz_path.display()))?;
    let mtime: DateTime<Utc> = metadata.modified()?.into();
    let fingerprint = utils::compute_fingerprint(&cbz_path, metadata.len(), &mtime)?;
    let format = BookFormat::Cbz;

    let mut tx = state.pool.begin().await?;
    merge_duplicate_file_row(&mut tx, file_id, book_id, &new_abs_path).await?;
    sqlx::query(
        "UPDATE book_files SET abs_path = $2, format = $3, size_bytes = $4, mtime = $5, \
         fingerprint = $6, updated_at = NOW() WHERE id = $1",
    )
    .bind(file_id)
    .bind(&new_abs_path)
    .bind(format.as_str())
    .bind(metadata.len() as i64)
    .bind(mtime)
    .bind(&fingerprint)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE books SET format = $2, kind = $3, updated_at = NOW() WHERE id = $1")
        .bind(book_id)
        .bind(format.as_str())
        .bind(utils::kind_from_format(format))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    info!("[CONVERTER] DB updated for book {}", book_id);

    // Delete the original CBR file (best-effort)
    if let Err(e) = std::fs::remove_file(cbr_path) {
        warn!(
            "[CONVERTER] Could not delete original CBR {}: {} (non-fatal)",
            cbr_path.display(),
            e
        );
    } else {
        info!("[CONVERTER] Deleted original CBR {}", cbr_path.display());
    }

    info!("[CONVERTER] Job {} converted book {}", job_id, book_id);
    Ok(book_name)
}

/// Free `new_abs_path` (UNIQUE in `book_files`) for the converted file row.
///
/// When a crash left the CBZ on disk, a scan may have indexed it as a distinct book before
/// this retry. That duplicate is folded into the original book: per-user reading progress is
/// carried over (the most recently updated row wins), then the duplicate file row is deleted
/// and its book too once it has no file left. Runs in the conversion transaction.
async fn merge_duplicate_file_row(
    tx: &mut Transaction<'_, Postgres>,
    file_id: Uuid,
    book_id: Uuid,
    new_abs_path: &str,
) -> Result<()> {
    let duplicate: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, book_id FROM book_files WHERE abs_path = $1 AND id <> $2 FOR UPDATE",
    )
    .bind(new_abs_path)
    .bind(file_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((dup_file_id, dup_book_id)) = duplicate else {
        return Ok(());
    };

    info!(
        "[CONVERTER] {} is already indexed as book {}: merging it into book {}",
        new_abs_path, dup_book_id, book_id
    );

    if dup_book_id != book_id {
        sqlx::query(
            "INSERT INTO book_reading_progress \
                 (book_id, user_id, status, current_page, last_read_at, updated_at) \
             SELECT $1, user_id, status, current_page, last_read_at, updated_at \
             FROM book_reading_progress WHERE book_id = $2 \
             ON CONFLICT (book_id, user_id) DO UPDATE \
               SET status = EXCLUDED.status, \
                   current_page = EXCLUDED.current_page, \
                   last_read_at = EXCLUDED.last_read_at, \
                   updated_at = EXCLUDED.updated_at \
               WHERE EXCLUDED.updated_at > book_reading_progress.updated_at",
        )
        .bind(book_id)
        .bind(dup_book_id)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query("DELETE FROM book_files WHERE id = $1")
        .bind(dup_file_id)
        .execute(&mut **tx)
        .await?;
    if dup_book_id != book_id {
        sqlx::query(
            "DELETE FROM books WHERE id = $1 \
             AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = $1)",
        )
        .bind(dup_book_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn create_library(pool: &PgPool) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(format!("lib-{id}"))
            .bind(format!("/tmp/{id}"))
            .execute(pool)
            .await
            .unwrap();
        id
    }

    async fn create_book(pool: &PgPool, library_id: Uuid, format: &str) -> Uuid {
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, kind, title, format) \
             VALUES ($1, $2, 'comic', 'Gon', $3)",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(format)
        .execute(pool)
        .await
        .unwrap();
        book_id
    }

    async fn add_book_file(
        pool: &PgPool,
        book_id: Uuid,
        format: &str,
        path: &std::path::Path,
    ) -> Uuid {
        let file_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint) \
             VALUES ($1, $2, $3, $4, 1024, NOW(), 'fp')",
        )
        .bind(file_id)
        .bind(book_id)
        .bind(format)
        .bind(path.to_string_lossy().to_string())
        .execute(pool)
        .await
        .unwrap();
        file_id
    }

    async fn create_job(pool: &PgPool) -> Uuid {
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) \
             VALUES ($1, NULL, 'cbr_to_cbz', 'running')",
        )
        .bind(job_id)
        .execute(pool)
        .await
        .unwrap();
        job_id
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn convert_book_errors_when_no_file_record(pool: PgPool) {
        let lib = create_library(&pool).await;
        let book = create_book(&pool, lib, "cbr").await;
        let job = create_job(&pool).await;

        let state = crate::test_state(pool.clone());
        let err = convert_book(&state, job, book).await.unwrap_err();
        assert!(
            err.to_string().contains("no book file found"),
            "unexpected error: {err}"
        );

        let events: Vec<(String, String)> =
            sqlx::query_as("SELECT event_type, level FROM index_job_events WHERE job_id = $1")
                .bind(job)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(events.len(), 1, "exactly one error event must be recorded");
        assert_eq!(events[0], ("error".to_string(), "error".to_string()));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn convert_book_rejects_non_cbr_format(pool: PgPool) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Gon T01.cbz");
        std::fs::write(&file, b"not used").unwrap();

        let lib = create_library(&pool).await;
        let book = create_book(&pool, lib, "cbz").await;
        add_book_file(&pool, book, "cbz", &file).await;
        let job = create_job(&pool).await;

        let state = crate::test_state(pool.clone());
        let err = convert_book(&state, job, book).await.unwrap_err();
        assert!(
            err.to_string().contains("is not CBR"),
            "a CBZ must not be converted: {err}"
        );

        let status: String = sqlx::query_scalar("SELECT status FROM index_jobs WHERE id = $1")
            .bind(job)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            status, "running",
            "a rejected conversion must not finish the job"
        );
        assert!(file.exists(), "a rejected book must be left untouched");
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn convert_book_reports_unrar_failure_without_touching_db(pool: PgPool) {
        let dir = tempfile::tempdir().unwrap();
        // A `.cbr` that is not a RAR archive: the in-process unrar reader fails.
        let file = dir.path().join("Gon T02.cbr");
        std::fs::write(&file, b"not a rar archive at all").unwrap();

        let lib = create_library(&pool).await;
        let book = create_book(&pool, lib, "cbr").await;
        let file_id = add_book_file(&pool, book, "cbr", &file).await;
        let job = create_job(&pool).await;

        let state = crate::test_state(pool.clone());
        let err = convert_book(&state, job, book).await.unwrap_err();
        assert!(
            err.to_string().contains("unrar"),
            "the unrar failure must be surfaced: {err}"
        );

        // The DB must not have been updated and the original file must survive.
        let (abs_path, format): (String, String) =
            sqlx::query_as("SELECT abs_path, format FROM book_files WHERE id = $1")
                .bind(file_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(format, "cbr");
        assert_eq!(abs_path, file.to_string_lossy());
        assert!(
            file.exists(),
            "the original CBR must not be deleted on failure"
        );
        assert!(
            !dir.path().join("Gon T02.cbz").exists(),
            "no CBZ must be produced when the conversion fails"
        );

        let error_events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM index_job_events WHERE job_id = $1 AND event_type = 'error'",
        )
        .bind(job)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(error_events, 1);
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    /// A crash after the CBZ was published but before the DB update leaves both files: the
    /// next run adopts the complete CBZ, updates `books.format` and the whole file row, and
    /// removes the CBR.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn convert_book_resumes_from_a_complete_cbz_left_by_a_crash(pool: PgPool) {
        let dir = tempfile::tempdir().unwrap();
        // The CBR is a mislabelled ZIP so its listing works without a RAR writer.
        let cbr = dir.path().join("Gon T03.cbr");
        let cbz = dir.path().join("Gon T03.cbz");
        let pages: &[(&str, &[u8])] = &[("ch1/01.jpg", b"a"), ("ch2/01.jpg", b"b")];
        write_zip(&cbr, pages);
        write_zip(&cbz, pages);

        let lib = create_library(&pool).await;
        let book = create_book(&pool, lib, "cbr").await;
        let file_id = add_book_file(&pool, book, "cbr", &cbr).await;
        let job = create_job(&pool).await;

        let state = crate::test_state(pool.clone());
        convert_book(&state, job, book).await.unwrap();

        let (abs_path, format, size, fingerprint): (String, String, i64, String) = sqlx::query_as(
            "SELECT abs_path, format, size_bytes, fingerprint FROM book_files WHERE id = $1",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let meta = std::fs::metadata(&cbz).unwrap();
        let mtime: DateTime<Utc> = meta.modified().unwrap().into();
        assert_eq!(abs_path, cbz.to_string_lossy());
        assert_eq!(format, "cbz");
        assert_eq!(size, meta.len() as i64);
        assert_eq!(
            fingerprint,
            utils::compute_fingerprint(&cbz, meta.len(), &mtime).unwrap(),
            "the next scan must not see the converted book as changed"
        );
        let book_format: String = sqlx::query_scalar("SELECT format FROM books WHERE id = $1")
            .bind(book)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(book_format, "cbz");
        assert!(!cbr.exists(), "the original CBR is removed");
        assert!(cbz.exists());
    }

    /// A different file already named like the target is never overwritten nor adopted.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn convert_book_refuses_an_unrelated_existing_cbz(pool: PgPool) {
        let dir = tempfile::tempdir().unwrap();
        let cbr = dir.path().join("Gon T04.cbr");
        let cbz = dir.path().join("Gon T04.cbz");
        write_zip(&cbr, &[("01.jpg", b"a"), ("02.jpg", b"b")]);
        write_zip(&cbz, &[("01.jpg", b"a")]);

        let lib = create_library(&pool).await;
        let book = create_book(&pool, lib, "cbr").await;
        let file_id = add_book_file(&pool, book, "cbr", &cbr).await;
        let job = create_job(&pool).await;

        let state = crate::test_state(pool.clone());
        let err = convert_book(&state, job, book).await.unwrap_err();

        assert!(err.to_string().contains("already exists"), "{err}");
        let format: String = sqlx::query_scalar("SELECT format FROM book_files WHERE id = $1")
            .bind(file_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(format, "cbr");
        assert!(cbr.exists());
    }

    /// After a crash the CBZ was indexed by a scan as a distinct book: the retry folds that
    /// duplicate into the original book (reading progress carried over, newest row wins)
    /// instead of failing forever on the UNIQUE `book_files.abs_path`.
    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn convert_book_merges_a_cbz_already_indexed_as_another_book(pool: PgPool) {
        let dir = tempfile::tempdir().unwrap();
        let cbr = dir.path().join("Gon T05.cbr");
        let cbz = dir.path().join("Gon T05.cbz");
        let pages: &[(&str, &[u8])] = &[("01.jpg", b"a"), ("02.jpg", b"b")];
        write_zip(&cbr, pages);
        write_zip(&cbz, pages);

        let lib = create_library(&pool).await;
        let book = create_book(&pool, lib, "cbr").await;
        let file_id = add_book_file(&pool, book, "cbr", &cbr).await;
        let duplicate = create_book(&pool, lib, "cbz").await;
        let dup_file = add_book_file(&pool, duplicate, "cbz", &cbz).await;

        let mut users = Vec::new();
        for name in ["only-dup", "dup-newer", "orig-newer"] {
            let id: Uuid =
                sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
                    .bind(name)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            users.push(id);
        }
        let progress = |book: Uuid, user: Uuid, status: &'static str, page: i32, age_min: i32| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO book_reading_progress \
                         (book_id, user_id, status, current_page, last_read_at, updated_at) \
                     VALUES ($1, $2, $3, $4, NOW(), NOW() - ($5 * INTERVAL '1 minute'))",
                )
                .bind(book)
                .bind(user)
                .bind(status)
                .bind(page)
                .bind(age_min)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        progress(duplicate, users[0], "reading", 7, 1).await;
        progress(book, users[1], "reading", 2, 10).await;
        progress(duplicate, users[1], "read", 30, 1).await;
        progress(book, users[2], "reading", 12, 1).await;
        progress(duplicate, users[2], "reading", 3, 10).await;
        let job = create_job(&pool).await;

        let state = crate::test_state(pool.clone());
        convert_book(&state, job, book).await.unwrap();

        let (abs_path, format): (String, String) =
            sqlx::query_as("SELECT abs_path, format FROM book_files WHERE id = $1")
                .bind(file_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            (abs_path.as_str(), format.as_str()),
            (&*cbz.to_string_lossy(), "cbz")
        );
        let leftovers: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM book_files WHERE id = $1) \
                  + (SELECT COUNT(*) FROM books WHERE id = $2)",
        )
        .bind(dup_file)
        .bind(duplicate)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(leftovers, 0, "the duplicate book and its file row are gone");

        let mut rows: Vec<(Uuid, String, i32)> = sqlx::query_as(
            "SELECT user_id, status, current_page FROM book_reading_progress WHERE book_id = $1",
        )
        .bind(book)
        .fetch_all(&pool)
        .await
        .unwrap();
        rows.sort_by_key(|(user, _, _)| users.iter().position(|u| u == user));
        assert_eq!(
            rows,
            vec![
                (users[0], "reading".to_string(), 7),
                (users[1], "read".to_string(), 30),
                (users[2], "reading".to_string(), 12),
            ]
        );
        assert!(!cbr.exists() && cbz.exists());
    }
}
