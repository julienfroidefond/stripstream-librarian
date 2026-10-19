use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{utils, AppState};

async fn insert_event(
    pool: &PgPool,
    job_id: Uuid,
    event_type: &str,
    level: &str,
    entity_name: Option<&str>,
    message: Option<&str>,
    detail: Option<serde_json::Value>,
) {
    let _ = sqlx::query(
        "INSERT INTO index_job_events (job_id, event_type, level, entity_name, message, detail) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(job_id)
    .bind(event_type)
    .bind(level)
    .bind(entity_name)
    .bind(message)
    .bind(detail)
    .execute(pool)
    .await;
}

/// Execute a `cbr_to_cbz` job for the given `book_id`.
///
/// Flow:
/// 1. Read book file info from DB
/// 2. Resolve physical path
/// 3. Convert CBR → CBZ via `parsers::convert_cbr_to_cbz`
/// 4. Update `book_files` and `books` in DB
/// 5. Delete the original CBR (failure here does not fail the job)
/// 6. Mark job as success
pub async fn convert_book(state: &AppState, job_id: Uuid, book_id: Uuid) -> Result<()> {
    info!(
        "[CONVERTER] Starting CBR→CBZ conversion for book {} (job {})",
        book_id, job_id
    );

    match convert_book_inner(state, job_id, book_id).await {
        Ok(book_name) => {
            insert_event(
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
            insert_event(
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

    let physical_path = utils::remap_libraries_path(&abs_path);
    let cbr_path = std::path::Path::new(&physical_path);

    let book_name = cbr_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| abs_path.clone());

    info!("[CONVERTER] Converting {} → CBZ", cbr_path.display());

    // Update job status to running (already set by claim_next_job, this updates current_file)
    sqlx::query("UPDATE index_jobs SET current_file = $2 WHERE id = $1")
        .bind(job_id)
        .bind(&abs_path)
        .execute(&state.pool)
        .await?;

    // Do the conversion
    let cbz_path = parsers::convert_cbr_to_cbz(cbr_path)?;

    info!("[CONVERTER] CBZ created at {}", cbz_path.display());

    // Remap physical path back to /libraries/ canonical form
    let new_abs_path = utils::unmap_libraries_path(&cbz_path.to_string_lossy());

    // Update book_files: abs_path + format
    sqlx::query(
        "UPDATE book_files SET abs_path = $2, format = 'cbz', updated_at = NOW() WHERE id = $1",
    )
    .bind(file_id)
    .bind(&new_abs_path)
    .execute(&state.pool)
    .await?;

    // Update books: kind stays 'comic', updated_at refreshed
    sqlx::query("UPDATE books SET updated_at = NOW() WHERE id = $1")
        .bind(book_id)
        .execute(&state.pool)
        .await?;

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

    // Mark job success
    sqlx::query(
        "UPDATE index_jobs SET status = 'success', finished_at = NOW(), progress_percent = 100, current_file = NULL WHERE id = $1",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;

    info!("[CONVERTER] Job {} completed successfully", job_id);
    Ok(book_name)
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let state = AppState { pool: pool.clone() };
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

        let state = AppState { pool: pool.clone() };
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

        let state = AppState { pool: pool.clone() };
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
}
