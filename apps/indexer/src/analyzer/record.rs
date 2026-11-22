//! What the analysis job writes once a batch of books has an outcome.
//!
//! One place owns the translation outcome → `analysis_status`, page count,
//! parse error, cover and event. Both sub-phases call it, and the event texts
//! are built here only: a message can no longer drift between the two copies.
//! The statuses themselves stay in `stripstream_core::analysis` (ADR 0025).

use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use stripstream_core::analysis;

use crate::batch::{clear_parse_errors, flush_events, flush_parse_errors, EventInsert};

use super::book::{Extracted, Published};

/// A cover staged by sub-phase A, waiting for sub-phase B.
#[derive(Clone)]
pub(super) struct StagedPage {
    pub book_id: Uuid,
    pub raw_path: String,
    pub page_count: i32,
}

/// Record a batch of sub-phase A outcomes and return the staged covers.
pub(super) async fn record_extraction(
    pool: &PgPool,
    job_id: Uuid,
    outcomes: Vec<Extracted>,
) -> anyhow::Result<Vec<StagedPage>> {
    let mut page_count_updates: Vec<(Uuid, i32)> = Vec::new();
    let mut parse_errors: Vec<(Uuid, String)> = Vec::new();
    // Archives read successfully this time (page count at least): any parse error of
    // a previous attempt is stale.
    let mut parsed_ok: Vec<Uuid> = Vec::new();
    let mut released: Vec<Uuid> = Vec::new();
    let mut failed: Vec<Uuid> = Vec::new();
    let mut events: Vec<EventInsert> = Vec::new();
    let mut resource_forks: Vec<Uuid> = Vec::new();
    let mut staged: Vec<StagedPage> = Vec::new();

    for outcome in outcomes {
        match outcome {
            Extracted::Skipped { book_id } => released.push(book_id),
            Extracted::ResourceFork { book_id } => resource_forks.push(book_id),
            Extracted::Failed {
                book_id,
                local_path,
                parse_error,
                message,
            } => {
                if let Some(parse_error) = parse_error {
                    parse_errors.push((book_id, parse_error));
                }
                failed.push(book_id);
                events.push(error_event(job_id, book_id, Some(local_path), message));
            }
            Extracted::PagesOnly {
                book_id,
                local_path,
                page_count,
                recovered_from,
            } => {
                page_count_updates.push((book_id, page_count));
                parsed_ok.push(book_id);
                events.push(pages_event(
                    job_id,
                    book_id,
                    Some(local_path),
                    page_count,
                    recovered_from.as_deref(),
                ));
            }
            Extracted::Staged {
                book_id,
                raw_path,
                page_count,
            } => {
                parsed_ok.push(book_id);
                staged.push(StagedPage {
                    book_id,
                    raw_path,
                    page_count,
                });
            }
        }
    }

    if !resource_forks.is_empty() {
        sqlx::query("DELETE FROM book_files WHERE book_id = ANY($1)")
            .bind(&resource_forks)
            .execute(pool)
            .await?;
        sqlx::query(
            "DELETE FROM books WHERE id = ANY($1) AND NOT EXISTS (SELECT 1 FROM book_files WHERE book_id = books.id)",
        )
        .bind(&resource_forks)
        .execute(pool)
        .await?;
    }

    if let Err(e) = analysis::record_page_counts(pool, &page_count_updates).await {
        warn!("[ANALYZER] Failed to record page counts: {e}");
    }
    if let Err(e) = flush_parse_errors(pool, &mut parse_errors).await {
        warn!("[ANALYZER] Failed to flush parse errors: {e}");
    }
    if let Err(e) = clear_parse_errors(pool, &mut parsed_ok).await {
        warn!("[ANALYZER] Failed to clear stale parse errors: {e}");
    }
    if let Err(e) = analysis::mark_failed(pool, &failed).await {
        warn!("[ANALYZER] Failed to record analysis failures: {e}");
    }
    if let Err(e) = analysis::release(pool, &released).await {
        warn!("[ANALYZER] Failed to release analysis claims: {e}");
    }
    if let Err(e) = flush_events(pool, &mut events).await {
        warn!("[ANALYZER] Failed to flush events: {e}");
    }

    Ok(staged)
}
/// Record a batch of sub-phase B outcomes: covers, recovered page counts, event.
pub(super) async fn record_publication(pool: &PgPool, job_id: Uuid, outcomes: Vec<Published>) {
    let mut thumb_updates: Vec<(Uuid, i32, String)> = Vec::new();
    let mut page_count_updates: Vec<(Uuid, i32)> = Vec::new();
    let mut events: Vec<EventInsert> = Vec::new();

    for outcome in outcomes {
        match outcome {
            Published::Cover {
                book_id,
                page_count,
                thumb_path,
            } => {
                events.push(thumbnail_event(job_id, book_id, &thumb_path));
                thumb_updates.push((book_id, page_count, thumb_path));
            }
            Published::PagesOnly {
                book_id,
                page_count,
                error,
            } => {
                events.push(pages_event(job_id, book_id, None, page_count, Some(&error)));
                page_count_updates.push((book_id, page_count));
            }
        }
    }

    if let Err(e) = analysis::record_covers(pool, &thumb_updates).await {
        warn!("[ANALYZER] Failed to record thumbnails: {e}");
    }
    if let Err(e) = analysis::record_page_counts(pool, &page_count_updates).await {
        warn!("[ANALYZER] Failed to record page counts: {e}");
    }
    if let Err(e) = flush_events(pool, &mut events).await {
        warn!("[ANALYZER] Failed to flush events: {e}");
    }
}

/// A page count was read. `recovered_from` carries the reason the cover could
/// not be produced: the same event is written by both sub-phases, so its text
/// and its detail live here.
fn pages_event(
    job_id: Uuid,
    book_id: Uuid,
    entity_name: Option<String>,
    page_count: i32,
    recovered_from: Option<&str>,
) -> EventInsert {
    let (message, level, detail) = match recovered_from {
        Some(error) => (
            format!(
                "Recovered {} pages, thumbnail skipped — {}",
                page_count, error
            ),
            "warning".to_string(),
            serde_json::json!({"page_count": page_count, "recovered": true, "error": error}),
        ),
        None => (
            format!("Extracted {} pages", page_count),
            "info".to_string(),
            serde_json::json!({"page_count": page_count}),
        ),
    };

    EventInsert {
        job_id,
        event_type: "pages_extracted".to_string(),
        level,
        entity_type: Some("book".to_string()),
        entity_id: Some(book_id),
        entity_name,
        message: Some(message),
        detail: Some(detail),
    }
}

fn error_event(
    job_id: Uuid,
    book_id: Uuid,
    entity_name: Option<String>,
    message: String,
) -> EventInsert {
    EventInsert {
        job_id,
        event_type: "error".to_string(),
        level: "error".to_string(),
        entity_type: Some("book".to_string()),
        entity_id: Some(book_id),
        entity_name,
        message: Some(message),
        detail: None,
    }
}

fn thumbnail_event(job_id: Uuid, book_id: Uuid, thumb_path: &str) -> EventInsert {
    EventInsert {
        job_id,
        event_type: "thumbnail_generated".to_string(),
        level: "info".to_string(),
        entity_type: Some("book".to_string()),
        entity_id: Some(book_id),
        entity_name: None,
        message: Some(format!("Thumbnail generated: {}", thumb_path)),
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recovered_page_count_keeps_its_error_in_the_event() {
        let event = pages_event(
            Uuid::nil(),
            Uuid::nil(),
            Some("/libraries/Gon T01.cbz".to_string()),
            12,
            Some("analyze_book timed out after 120s"),
        );

        assert_eq!(event.event_type, "pages_extracted");
        assert_eq!(event.level, "warning");
        assert_eq!(
            event.message.as_deref(),
            Some("Recovered 12 pages, thumbnail skipped — analyze_book timed out after 120s")
        );
        assert_eq!(event.detail.unwrap()["recovered"], serde_json::json!(true));
    }

    #[test]
    fn a_plain_page_count_is_an_info_event() {
        let event = pages_event(Uuid::nil(), Uuid::nil(), None, 30, None);

        assert_eq!(event.level, "info");
        assert_eq!(event.message.as_deref(), Some("Extracted 30 pages"));
        assert_eq!(event.detail.unwrap()["page_count"], serde_json::json!(30));
    }

    #[test]
    fn a_published_cover_events_the_path() {
        let event = thumbnail_event(Uuid::nil(), Uuid::nil(), "/data/thumbnails/x.webp");

        assert_eq!(event.event_type, "thumbnail_generated");
        assert_eq!(
            event.message.as_deref(),
            Some("Thumbnail generated: /data/thumbnails/x.webp")
        );
    }
}

/// The translation outcome → database is the seam: these tests drive it with
/// hand-built outcomes, without opening a single archive.
#[cfg(test)]
mod db_tests {
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::{record_extraction, record_publication};
    use crate::analyzer::book::{Extracted, Published};

    struct Book {
        pool: PgPool,
        id: Uuid,
        job_id: Uuid,
    }

    impl Book {
        /// A book claimed by a running analysis job, as the pipeline leaves it.
        async fn analyzing(pool: PgPool) -> Self {
            let library_id = Uuid::new_v4();
            sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
                .bind(library_id)
                .bind(format!("lib-{library_id}"))
                .bind(format!("/tmp/{library_id}"))
                .execute(&pool)
                .await
                .unwrap();
            let series_id = Uuid::new_v4();
            sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Gon')")
                .bind(series_id)
                .bind(library_id)
                .execute(&pool)
                .await
                .unwrap();
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO books (id, library_id, series_id, kind, title, format, analysis_status) \
                 VALUES ($1, $2, $3, 'comic', 'Gon T01', 'cbz', 'analyzing')",
            )
            .bind(id)
            .bind(library_id)
            .bind(series_id)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO book_files (id, book_id, format, abs_path, size_bytes, mtime, fingerprint) \
                 VALUES ($1, $2, 'cbz', '/libraries/Gon/Gon T01.cbz', 1024, NOW(), 'fp')",
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

            let job_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO index_jobs (id, library_id, type, status) \
                 VALUES ($1, NULL, 'scan', 'running')",
            )
            .bind(job_id)
            .execute(&pool)
            .await
            .unwrap();

            Self { pool, id, job_id }
        }

        async fn status(&self) -> (Option<i32>, String) {
            sqlx::query_as("SELECT page_count, analysis_status FROM books WHERE id = $1")
                .bind(self.id)
                .fetch_one(&self.pool)
                .await
                .unwrap()
        }
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_recovered_page_count_makes_the_book_cover_pending(pool: PgPool) {
        let book = Book::analyzing(pool).await;

        let staged = record_extraction(
            &book.pool,
            book.job_id,
            vec![Extracted::PagesOnly {
                book_id: book.id,
                local_path: "/libraries/Gon/Gon T01.cbz".to_string(),
                page_count: 12,
                recovered_from: Some("analyze_book timed out after 120s".to_string()),
            }],
        )
        .await
        .unwrap();

        assert!(staged.is_empty());
        assert_eq!(book.status().await, (Some(12), "cover_pending".to_string()));

        let (event_type, level, message): (String, String, String) = sqlx::query_as(
            "SELECT event_type, level, message FROM index_job_events \
             WHERE job_id = $1 AND entity_id = $2",
        )
        .bind(book.job_id)
        .bind(book.id)
        .fetch_one(&book.pool)
        .await
        .unwrap();
        assert_eq!(event_type, "pages_extracted");
        assert_eq!(level, "warning");
        assert_eq!(
            message,
            "Recovered 12 pages, thumbnail skipped — analyze_book timed out after 120s"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_failure_records_the_parse_error_and_the_failed_state(pool: PgPool) {
        let book = Book::analyzing(pool).await;

        record_extraction(
            &book.pool,
            book.job_id,
            vec![Extracted::Failed {
                book_id: book.id,
                local_path: "/libraries/Gon/Gon T01.cbz".to_string(),
                parse_error: Some("not a zip".to_string()),
                message: "Extraction failed: Gon T01.cbz".to_string(),
            }],
        )
        .await
        .unwrap();

        assert_eq!(book.status().await, (None, "failed".to_string()));
        let (parse_status, parse_error): (String, Option<String>) = sqlx::query_as(
            "SELECT parse_status, parse_error_opt FROM book_files WHERE book_id = $1",
        )
        .bind(book.id)
        .fetch_one(&book.pool)
        .await
        .unwrap();
        assert_eq!(parse_status, "error");
        assert_eq!(parse_error.as_deref(), Some("not a zip"));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_resource_fork_removes_the_book_and_its_files(pool: PgPool) {
        let book = Book::analyzing(pool).await;

        record_extraction(
            &book.pool,
            book.job_id,
            vec![Extracted::ResourceFork { book_id: book.id }],
        )
        .await
        .unwrap();

        let books: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE id = $1")
            .bind(book.id)
            .fetch_one(&book.pool)
            .await
            .unwrap();
        let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM book_files WHERE book_id = $1")
            .bind(book.id)
            .fetch_one(&book.pool)
            .await
            .unwrap();
        assert_eq!((books, files), (0, 0));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn a_staged_page_waits_for_sub_phase_b(pool: PgPool) {
        let book = Book::analyzing(pool).await;

        let staged = record_extraction(
            &book.pool,
            book.job_id,
            vec![Extracted::Staged {
                book_id: book.id,
                raw_path: "/data/thumbnails/x.raw".to_string(),
                page_count: 30,
            }],
        )
        .await
        .unwrap();

        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].raw_path, "/data/thumbnails/x.raw");
        // Still `analyzing`: the page count and the cover land together in B.
        assert_eq!(book.status().await, (None, "analyzing".to_string()));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn publishing_a_cover_completes_the_book(pool: PgPool) {
        let book = Book::analyzing(pool).await;

        record_publication(
            &book.pool,
            book.job_id,
            vec![Published::Cover {
                book_id: book.id,
                page_count: 30,
                thumb_path: "/data/thumbnails/x.webp".to_string(),
            }],
        )
        .await;

        assert_eq!(book.status().await, (Some(30), "complete".to_string()));
        let message: String = sqlx::query_scalar(
            "SELECT message FROM index_job_events WHERE job_id = $1 AND entity_id = $2",
        )
        .bind(book.job_id)
        .bind(book.id)
        .fetch_one(&book.pool)
        .await
        .unwrap();
        assert_eq!(message, "Thumbnail generated: /data/thumbnails/x.webp");
    }
}
