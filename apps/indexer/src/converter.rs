use anyhow::Result;
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{utils, AppState};

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
    info!("[CONVERTER] Starting CBR→CBZ conversion for book {} (job {})", book_id, job_id);

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

    info!("[CONVERTER] Converting {} → CBZ", cbr_path.display());

    // Update job status to running (already set by claim_next_job, this updates current_file)
    sqlx::query(
        "UPDATE index_jobs SET current_file = $2 WHERE id = $1",
    )
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
    Ok(())
}
