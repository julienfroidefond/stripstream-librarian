//! The only writer of discovery results: resolves series through the scan cache, buffers rows
//! in a [`ScanBatch`] and records the matching job events.

use std::collections::HashMap;

use anyhow::Result;
use parsers::ParsedMetadata;
use sqlx::PgPool;
use stripstream_core::analysis::AnalysisStatus;
use stripstream_core::series_identity;
use tracing::debug;
use uuid::Uuid;

use crate::batch::{
    BookCorrection, BookInsert, BookUpdate, EventInsert, FileInsert, FileUpdate, ScanBatch,
};
use crate::utils;

use super::reconcile::{FileStat, Reconciliation};
use super::{JobStats, BATCH_SIZE};

pub struct ScanWriter {
    pool: PgPool,
    job_id: Uuid,
    library_id: Uuid,
    /// Raw series name → id, seeded with the library's current names. Keys are the raw
    /// spellings seen during the scan; identity matching stays in `series_identity`.
    series_cache: HashMap<String, Uuid>,
    batch: ScanBatch,
}

impl ScanWriter {
    pub fn new(
        pool: PgPool,
        job_id: Uuid,
        library_id: Uuid,
        series_cache: HashMap<String, Uuid>,
    ) -> Self {
        Self {
            pool,
            job_id,
            library_id,
            series_cache,
            batch: ScanBatch::default(),
        }
    }

    /// Write a reconciliation for a known file. `abs_path` is the database path recorded on
    /// the event; `file_name` labels it.
    pub async fn apply(
        &mut self,
        reconciliation: Reconciliation,
        abs_path: &str,
        file_name: &str,
        stats: &mut JobStats,
    ) -> Result<()> {
        match reconciliation {
            Reconciliation::Noop => return Ok(()),
            Reconciliation::Correct(c) => {
                debug!(
                    "[SCAN] Title/volume/type mismatch for {}: DB=('{}', {:?}) vs parsed=('{}', {:?}, '{}'), updating",
                    file_name,
                    c.previous_title,
                    c.previous_volume,
                    c.title,
                    c.volume,
                    c.volume_type.as_str()
                );
                let series_id = self.series_id(c.series.as_deref()).await?;
                let message = format!(
                    "Title/volume updated: ('{}', {:?}) → ('{}', {:?})",
                    c.previous_title, c.previous_volume, c.title, c.volume
                );
                self.batch.book_corrections.push(BookCorrection {
                    book_id: c.book_id,
                    title: c.title,
                    volume: c.volume,
                    volume_type: c.volume_type.as_str().to_string(),
                    series_id,
                });
                self.event("book_updated", c.book_id, abs_path, message);
            }
            Reconciliation::Reindex(r) => {
                debug!(target: "scan", "[SCAN] Updating: {} (fingerprint changed)", file_name);
                let series_id = self.series_id(r.series.as_deref()).await?;
                let format = r.file.format;
                self.batch.books_update.push(BookUpdate {
                    book_id: r.book_id,
                    title: r.title,
                    kind: utils::kind_from_format(format).to_string(),
                    format: format.as_str().to_string(),
                    series_id,
                    volume: r.volume,
                    volume_type: r.volume_type.as_str().to_string(),
                    // Reset page_count so the analyzer re-processes this book
                    page_count: None,
                    clear_thumbnail: true,
                    analysis_status: AnalysisStatus::Discovered,
                });
                self.batch.files_update.push(FileUpdate {
                    file_id: r.file_id,
                    format: format.as_str().to_string(),
                    size_bytes: r.file.size_bytes,
                    mtime: r.file.mtime,
                    fingerprint: r.file.fingerprint,
                });
                self.event(
                    "book_updated",
                    r.book_id,
                    abs_path,
                    format!("Book updated (fingerprint changed): {file_name}"),
                );
                stats.indexed_files += 1;
            }
        }
        self.flush_if_full().await
    }

    /// Insert a newly discovered file; the analyzer fills page count and cover later.
    pub async fn insert_new(
        &mut self,
        parsed: ParsedMetadata,
        file: FileStat,
        abs_path: &str,
        file_name: &str,
        stats: &mut JobStats,
    ) -> Result<()> {
        let book_id = Uuid::new_v4();
        let series_id = self.series_id(parsed.series.as_deref()).await?;
        self.batch.books_insert.push(BookInsert {
            book_id,
            library_id: self.library_id,
            kind: utils::kind_from_format(file.format).to_string(),
            format: file.format.as_str().to_string(),
            title: parsed.title,
            series_id,
            volume: parsed.volume,
            volume_type: parsed.volume_type.as_str().to_string(),
            page_count: None,
            thumbnail_path: None,
            analysis_status: AnalysisStatus::Discovered,
        });
        self.batch.files_insert.push(FileInsert {
            file_id: Uuid::new_v4(),
            book_id,
            format: file.format.as_str().to_string(),
            abs_path: abs_path.to_string(),
            size_bytes: file.size_bytes,
            mtime: file.mtime,
            fingerprint: file.fingerprint,
            parse_status: "ok".to_string(),
            parse_error: None,
        });
        self.event(
            "book_added",
            book_id,
            abs_path,
            format!("New book discovered: {file_name}"),
        );
        stats.indexed_files += 1;
        self.flush_if_full().await
    }

    /// Write everything still buffered.
    pub async fn flush(&mut self) -> Result<()> {
        self.batch.flush(&self.pool).await
    }

    async fn flush_if_full(&mut self) -> Result<()> {
        if self.batch.is_full(BATCH_SIZE) {
            self.flush().await?;
        }
        Ok(())
    }

    async fn series_id(&mut self, name: Option<&str>) -> Result<Option<Uuid>> {
        let Some(name) = name else {
            return Ok(None);
        };
        if let Some(&id) = self.series_cache.get(name) {
            return Ok(Some(id));
        }
        let identity = series_identity::find_or_create(&self.pool, self.library_id, name).await?;
        self.series_cache.insert(name.to_string(), identity.id);
        Ok(Some(identity.id))
    }

    fn event(&mut self, event_type: &str, book_id: Uuid, abs_path: &str, message: String) {
        self.batch.events.push(EventInsert {
            job_id: self.job_id,
            event_type: event_type.to_string(),
            level: "info".to_string(),
            entity_type: Some("book".to_string()),
            entity_id: Some(book_id),
            entity_name: Some(abs_path.to_string()),
            message: Some(message),
            detail: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use parsers::{BookFormat, VolumeType};

    use super::*;
    use crate::scanner::reconcile::{Correction, Reindex};

    struct Fixture {
        library_id: Uuid,
        job_id: Uuid,
        series_id: Uuid,
        book_id: Uuid,
        file_id: Uuid,
    }

    async fn fixture(pool: &PgPool) -> Fixture {
        let library_id = Uuid::new_v4();
        sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, 'w', '/libraries/w')")
            .bind(library_id)
            .execute(pool)
            .await
            .unwrap();
        let series_id = Uuid::new_v4();
        sqlx::query("INSERT INTO series (id, library_id, name) VALUES ($1, $2, 'Series')")
            .bind(series_id)
            .bind(library_id)
            .execute(pool)
            .await
            .unwrap();
        let book_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, title, kind, format, series_id, thumbnail_path, page_count, analysis_status) \
             VALUES ($1, $2, 'Old', 'comic', 'cbz', $3, '/thumbs/x.webp', 10, 'complete')",
        )
        .bind(book_id)
        .bind(library_id)
        .bind(series_id)
        .execute(pool)
        .await
        .unwrap();
        let file_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO book_files (id, book_id, abs_path, format, size_bytes, mtime, fingerprint, parse_status) \
             VALUES ($1, $2, '/libraries/w/Series/Series - T05.cbz', 'cbz', 1, NOW(), 'fp_old', 'ok')",
        )
        .bind(file_id)
        .bind(book_id)
        .execute(pool)
        .await
        .unwrap();
        let job_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO index_jobs (id, library_id, type, status) VALUES ($1, $2, 'scan', 'running')",
        )
        .bind(job_id)
        .bind(library_id)
        .execute(pool)
        .await
        .unwrap();
        Fixture {
            library_id,
            job_id,
            series_id,
            book_id,
            file_id,
        }
    }

    fn stats() -> JobStats {
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

    type BookRow = (
        String,
        Option<i32>,
        String,
        Option<Uuid>,
        Option<String>,
        String,
    );

    async fn book(pool: &PgPool, id: Uuid) -> BookRow {
        sqlx::query_as(
            "SELECT title, volume, volume_type, series_id, thumbnail_path, analysis_status \
             FROM books WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn events(pool: &PgPool, job_id: Uuid) -> Vec<(String, Option<String>)> {
        sqlx::query_as("SELECT event_type, message FROM index_job_events WHERE job_id = $1")
            .bind(job_id)
            .fetch_all(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn apply_noop_writes_nothing(pool: PgPool) {
        let f = fixture(&pool).await;
        let mut writer = ScanWriter::new(pool.clone(), f.job_id, f.library_id, HashMap::new());
        let mut stats = stats();

        writer
            .apply(Reconciliation::Noop, "/p", "p.cbz", &mut stats)
            .await
            .unwrap();
        writer.flush().await.unwrap();

        assert_eq!(book(&pool, f.book_id).await.0, "Old");
        assert!(events(&pool, f.job_id).await.is_empty());
        assert_eq!(stats.indexed_files, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn apply_correct_batches_title_and_keeps_analysis(pool: PgPool) {
        let f = fixture(&pool).await;
        let mut writer = ScanWriter::new(pool.clone(), f.job_id, f.library_id, HashMap::new());
        let mut stats = stats();
        let correction = Correction {
            book_id: f.book_id,
            previous_title: "Old".to_string(),
            previous_volume: None,
            title: "Series - T05".to_string(),
            volume: Some(5),
            volume_type: VolumeType::Hs,
            series: None,
        };

        writer
            .apply(
                Reconciliation::Correct(correction),
                "/p",
                "p.cbz",
                &mut stats,
            )
            .await
            .unwrap();
        assert_eq!(
            book(&pool, f.book_id).await.0,
            "Old",
            "buffered until flush"
        );
        writer.flush().await.unwrap();

        assert_eq!(
            book(&pool, f.book_id).await,
            (
                "Series - T05".to_string(),
                Some(5),
                "hs".to_string(),
                Some(f.series_id),
                Some("/thumbs/x.webp".to_string()),
                "complete".to_string()
            ),
            "no series name keeps series_id; analysis untouched"
        );
        assert_eq!(
            events(&pool, f.job_id).await,
            vec![(
                "book_updated".to_string(),
                Some("Title/volume updated: ('Old', None) → ('Series - T05', Some(5))".to_string())
            )]
        );
        assert_eq!(stats.indexed_files, 0);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn apply_correct_resolves_series_name(pool: PgPool) {
        let f = fixture(&pool).await;
        let mut writer = ScanWriter::new(pool.clone(), f.job_id, f.library_id, HashMap::new());
        let correction = Correction {
            book_id: f.book_id,
            previous_title: "Old".to_string(),
            previous_volume: None,
            title: "Other - T01".to_string(),
            volume: Some(1),
            volume_type: VolumeType::Regular,
            series: Some("Other".to_string()),
        };

        writer
            .apply(
                Reconciliation::Correct(correction),
                "/p",
                "p.cbz",
                &mut stats(),
            )
            .await
            .unwrap();
        writer.flush().await.unwrap();

        let other: Uuid =
            sqlx::query_scalar("SELECT id FROM series WHERE library_id = $1 AND name = 'Other'")
                .bind(f.library_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(book(&pool, f.book_id).await.3, Some(other));
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn series_names_resolve_through_series_identity(pool: PgPool) {
        let f = fixture(&pool).await;
        let mut writer = ScanWriter::new(pool.clone(), f.job_id, f.library_id, HashMap::new());

        // Other spellings of the existing series reuse it instead of creating one.
        for name in ["Series", "series", "SÉRIES"] {
            assert_eq!(
                writer.series_id(Some(name)).await.unwrap(),
                Some(f.series_id),
                "{name} must resolve to the existing series"
            );
        }
        assert_eq!(writer.series_id(None).await.unwrap(), None);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM series WHERE library_id = $1")
            .bind(f.library_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn apply_reindex_resets_analysis_and_updates_file(pool: PgPool) {
        let f = fixture(&pool).await;
        let mut writer = ScanWriter::new(pool.clone(), f.job_id, f.library_id, HashMap::new());
        let mut stats = stats();
        let reindex = Reindex {
            book_id: f.book_id,
            file_id: f.file_id,
            title: "Series - T05".to_string(),
            volume: Some(5),
            volume_type: VolumeType::Regular,
            series: Some("Series".to_string()),
            file: FileStat {
                format: BookFormat::Cbz,
                size_bytes: 4096,
                mtime: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                fingerprint: "fp_new".to_string(),
            },
        };

        writer
            .apply(Reconciliation::Reindex(reindex), "/p", "p.cbz", &mut stats)
            .await
            .unwrap();
        writer.flush().await.unwrap();

        assert_eq!(
            book(&pool, f.book_id).await,
            (
                "Series - T05".to_string(),
                Some(5),
                "regular".to_string(),
                Some(f.series_id),
                None,
                "discovered".to_string()
            )
        );
        let (fingerprint, size): (String, i64) =
            sqlx::query_as("SELECT fingerprint, size_bytes FROM book_files WHERE id = $1")
                .bind(f.file_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((fingerprint.as_str(), size), ("fp_new", 4096));
        assert_eq!(
            events(&pool, f.job_id).await,
            vec![(
                "book_updated".to_string(),
                Some("Book updated (fingerprint changed): p.cbz".to_string())
            )]
        );
        assert_eq!(stats.indexed_files, 1);
    }
}
