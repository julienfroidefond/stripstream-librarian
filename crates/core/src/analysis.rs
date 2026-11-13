//! Explicit lifecycle of a book's archive analysis (`books.analysis_status`).
//!
//! This module is the only writer of `analysis_status` outside of discovery
//! (which inserts or re-indexes books as [`AnalysisStatus::Discovered`]):
//!
//! | Transition | From | To |
//! |---|---|---|
//! | [`claim`] (full analysis) | `discovered`, `analyzing`, `failed` | `analyzing` |
//! | [`release`] (skipped: cancellation, unknown format) | `analyzing` | `discovered` |
//! | [`mark_failed`] (no page count) | any | `failed` |
//! | [`record_page_counts`] | any | `complete` if a cover is stored, else `cover_pending` |
//! | [`record_covers`] (indexer: page count + cover) | any | `complete` |
//! | [`record_cover`] (API on-the-fly render) | `cover_pending` with a page count | `complete` (other states keep theirs) |
//! | [`reset_covers`] (thumbnail regeneration) | `complete` with a page count | `cover_pending`, cover cleared |
//!
//! `analyzing` is a valid work input again: a crashed or cancelled job leaves
//! rows in that state and the next full analysis retries them. A cover-only job
//! does not take a claim: its books stay `cover_pending` until a cover is
//! recorded, so a failed resize is retried by the next job.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    /// Indexed by discovery, archive never opened.
    Discovered,
    /// Claimed by an analysis job that has not recorded an outcome yet.
    Analyzing,
    /// Page count known, thumbnail still to generate.
    CoverPending,
    /// Page count and thumbnail both known.
    Complete,
    /// Analysis produced no page count; retryable.
    Failed,
}

impl AnalysisStatus {
    pub const ALL: [Self; 5] = [
        Self::Discovered,
        Self::Analyzing,
        Self::CoverPending,
        Self::Complete,
        Self::Failed,
    ];

    /// Stable database representation used by `books.analysis_status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Analyzing => "analyzing",
            Self::CoverPending => "cover_pending",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    /// Parse a value read from `books.analysis_status`.
    pub fn from_db(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
    }
}

/// What an analysis job works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisMode {
    /// Open the archive: page count and cover (`discovered`, `analyzing`, `failed`).
    Full,
    /// Generate the missing cover of books whose page count is known (`cover_pending`).
    CoverOnly,
}

impl AnalysisMode {
    /// States this mode selects.
    pub const fn selects(self) -> &'static [AnalysisStatus] {
        match self {
            Self::Full => &[
                AnalysisStatus::Discovered,
                AnalysisStatus::Analyzing,
                AnalysisStatus::Failed,
            ],
            Self::CoverOnly => &[AnalysisStatus::CoverPending],
        }
    }

    /// [`Self::selects`] as database values, to bind as `text[]`
    /// (`analysis_status = ANY($n)`).
    pub fn selected_values(self) -> Vec<String> {
        self.selects()
            .iter()
            .map(|status| status.as_str().to_string())
            .collect()
    }
}

/// Claim books for a running job. A full analysis moves the selectable books to
/// `analyzing`; books that advanced concurrently (`complete`, `cover_pending`
/// after an API render) are left alone. A cover-only job takes no claim.
pub async fn claim(pool: &PgPool, book_ids: &[Uuid], mode: AnalysisMode) -> sqlx::Result<()> {
    if book_ids.is_empty() || mode == AnalysisMode::CoverOnly {
        return Ok(());
    }
    sqlx::query(
        "UPDATE books SET analysis_status = 'analyzing', updated_at = NOW() \
         WHERE id = ANY($1) AND analysis_status = ANY($2)",
    )
    .bind(book_ids)
    .bind(mode.selected_values())
    .execute(pool)
    .await
    .map(|_| ())
}

/// Release claims without a verdict (cancellation, unknown format): `analyzing`
/// goes back to `discovered`; any other state is kept.
pub async fn release(pool: &PgPool, book_ids: &[Uuid]) -> sqlx::Result<()> {
    if book_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE books SET analysis_status = 'discovered', updated_at = NOW() \
         WHERE id = ANY($1) AND analysis_status = 'analyzing'",
    )
    .bind(book_ids)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Record that no page count could be read: `failed`, retried by the next full
/// analysis.
pub async fn mark_failed(pool: &PgPool, book_ids: &[Uuid]) -> sqlx::Result<()> {
    if book_ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE books SET analysis_status = 'failed', updated_at = NOW() WHERE id = ANY($1)",
    )
    .bind(book_ids)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Record page counts without a new cover. The state follows the cover stored
/// at write time (it may have been rendered concurrently by the API):
/// `complete` with a cover, `cover_pending` without.
pub async fn record_page_counts(pool: &PgPool, page_counts: &[(Uuid, i32)]) -> sqlx::Result<()> {
    if page_counts.is_empty() {
        return Ok(());
    }
    let book_ids: Vec<Uuid> = page_counts.iter().map(|(id, _)| *id).collect();
    let counts: Vec<i32> = page_counts.iter().map(|(_, count)| *count).collect();
    sqlx::query(
        r#"
        UPDATE books SET
            page_count = data.page_count,
            analysis_status = CASE
                WHEN books.thumbnail_path IS NULL THEN 'cover_pending'
                ELSE 'complete'
            END,
            updated_at = NOW()
        FROM UNNEST($1::uuid[], $2::int[]) AS data(book_id, page_count)
        WHERE books.id = data.book_id
        "#,
    )
    .bind(&book_ids)
    .bind(&counts)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Record covers generated by the indexer together with their page count:
/// the books are `complete`.
pub async fn record_covers(pool: &PgPool, covers: &[(Uuid, i32, String)]) -> sqlx::Result<()> {
    if covers.is_empty() {
        return Ok(());
    }
    let book_ids: Vec<Uuid> = covers.iter().map(|(id, _, _)| *id).collect();
    let counts: Vec<i32> = covers.iter().map(|(_, count, _)| *count).collect();
    let paths: Vec<String> = covers.iter().map(|(_, _, path)| path.clone()).collect();
    sqlx::query(
        r#"
        UPDATE books SET
            page_count = data.page_count,
            thumbnail_path = data.thumbnail_path,
            analysis_status = 'complete',
            updated_at = NOW()
        FROM UNNEST($1::uuid[], $2::int[], $3::text[]) AS data(book_id, page_count, thumbnail_path)
        WHERE books.id = data.book_id
        "#,
    )
    .bind(&book_ids)
    .bind(&counts)
    .bind(&paths)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Record a cover rendered outside of an analysis job (API). A `cover_pending`
/// book with a page count becomes `complete`; any other state is kept so a
/// running analysis still records its own outcome.
pub async fn record_cover(pool: &PgPool, book_id: Uuid, path: &str) -> sqlx::Result<()> {
    sqlx::query(
        r#"
        UPDATE books SET
            thumbnail_path = $1,
            analysis_status = CASE
                WHEN analysis_status = 'cover_pending' AND page_count IS NOT NULL THEN 'complete'
                ELSE analysis_status
            END,
            updated_at = NOW()
        WHERE id = $2
        "#,
    )
    .bind(path)
    .bind(book_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Clear the covers of `complete` books (one library, or all when `None`) so a
/// cover-only job regenerates them, and return their ids for the file purge.
/// Books without a page count are left alone: their cover could not be rebuilt.
pub async fn reset_covers(pool: &PgPool, library_id: Option<Uuid>) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar(
        r#"
        UPDATE books SET
            thumbnail_path = NULL,
            analysis_status = 'cover_pending',
            updated_at = NOW()
        WHERE (library_id = $1 OR $1 IS NULL)
          AND analysis_status = 'complete'
          AND page_count IS NOT NULL
        RETURNING id
        "#,
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_statuses_round_trip_through_the_database_value() {
        for status in AnalysisStatus::ALL {
            assert_eq!(AnalysisStatus::from_db(status.as_str()), Some(status));
        }
        assert_eq!(AnalysisStatus::from_db("unknown"), None);
    }

    #[test]
    fn modes_select_disjoint_states() {
        assert_eq!(
            AnalysisMode::Full.selected_values(),
            ["discovered", "analyzing", "failed"]
        );
        assert_eq!(AnalysisMode::CoverOnly.selected_values(), ["cover_pending"]);
    }

    async fn library(pool: &PgPool) -> Uuid {
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

    async fn book(
        pool: &PgPool,
        library_id: Uuid,
        status: AnalysisStatus,
        page_count: Option<i32>,
        thumbnail: Option<&str>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO books (id, library_id, kind, title, format, page_count, thumbnail_path, \
                                analysis_status) \
             VALUES ($1, $2, 'comic', 'Book', 'cbz', $3, $4, $5)",
        )
        .bind(id)
        .bind(library_id)
        .bind(page_count)
        .bind(thumbnail)
        .bind(status.as_str())
        .execute(pool)
        .await
        .unwrap();
        id
    }

    async fn state(pool: &PgPool, id: Uuid) -> (AnalysisStatus, Option<i32>, Option<String>) {
        let (status, page_count, thumbnail): (String, Option<i32>, Option<String>) =
            sqlx::query_as(
                "SELECT analysis_status, page_count, thumbnail_path FROM books WHERE id = $1",
            )
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
        (
            AnalysisStatus::from_db(&status).unwrap(),
            page_count,
            thumbnail,
        )
    }

    async fn status(pool: &PgPool, id: Uuid) -> AnalysisStatus {
        state(pool, id).await.0
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn claim_takes_full_analysis_candidates_only(pool: PgPool) {
        use AnalysisStatus::*;
        let lib = library(&pool).await;
        let mut books = Vec::new();
        for s in AnalysisStatus::ALL {
            books.push((s, book(&pool, lib, s, None, None).await));
        }
        let ids: Vec<Uuid> = books.iter().map(|(_, id)| *id).collect();

        claim(&pool, &ids, AnalysisMode::CoverOnly).await.unwrap();
        for (s, id) in &books {
            assert_eq!(status(&pool, *id).await, *s, "cover-only takes no claim");
        }

        claim(&pool, &ids, AnalysisMode::Full).await.unwrap();
        for (s, id) in &books {
            let expected = match s {
                Discovered | Analyzing | Failed => Analyzing,
                other => *other,
            };
            assert_eq!(status(&pool, *id).await, expected, "from {s:?}");
        }
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn release_only_returns_claimed_books_to_discovered(pool: PgPool) {
        let lib = library(&pool).await;
        let claimed = book(&pool, lib, AnalysisStatus::Analyzing, None, None).await;
        let cover_pending = book(&pool, lib, AnalysisStatus::CoverPending, Some(3), None).await;

        release(&pool, &[claimed, cover_pending]).await.unwrap();

        assert_eq!(status(&pool, claimed).await, AnalysisStatus::Discovered);
        assert_eq!(
            status(&pool, cover_pending).await,
            AnalysisStatus::CoverPending,
            "a skipped cover-only task keeps its known page count"
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn mark_failed_records_a_failure(pool: PgPool) {
        let lib = library(&pool).await;
        let id = book(&pool, lib, AnalysisStatus::Analyzing, None, None).await;

        mark_failed(&pool, &[id]).await.unwrap();

        assert_eq!(status(&pool, id).await, AnalysisStatus::Failed);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn page_counts_follow_the_stored_cover(pool: PgPool) {
        let lib = library(&pool).await;
        let without_cover = book(&pool, lib, AnalysisStatus::Analyzing, None, None).await;
        let with_cover = book(
            &pool,
            lib,
            AnalysisStatus::Analyzing,
            None,
            Some("/t/a.webp"),
        )
        .await;

        record_page_counts(&pool, &[(without_cover, 12), (with_cover, 8)])
            .await
            .unwrap();

        assert_eq!(
            state(&pool, without_cover).await,
            (AnalysisStatus::CoverPending, Some(12), None)
        );
        assert_eq!(
            state(&pool, with_cover).await,
            (
                AnalysisStatus::Complete,
                Some(8),
                Some("/t/a.webp".to_string())
            )
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn indexer_covers_complete_the_book(pool: PgPool) {
        let lib = library(&pool).await;
        let id = book(&pool, lib, AnalysisStatus::Analyzing, None, None).await;

        record_covers(&pool, &[(id, 20, "/t/b.webp".to_string())])
            .await
            .unwrap();

        assert_eq!(
            state(&pool, id).await,
            (
                AnalysisStatus::Complete,
                Some(20),
                Some("/t/b.webp".to_string())
            )
        );
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn an_api_cover_completes_only_a_cover_pending_book(pool: PgPool) {
        let lib = library(&pool).await;
        let pending = book(&pool, lib, AnalysisStatus::CoverPending, Some(5), None).await;
        let discovered = book(&pool, lib, AnalysisStatus::Discovered, None, None).await;
        let analyzing = book(&pool, lib, AnalysisStatus::Analyzing, None, None).await;

        for id in [pending, discovered, analyzing] {
            record_cover(&pool, id, "/t/c.webp").await.unwrap();
        }

        assert_eq!(status(&pool, pending).await, AnalysisStatus::Complete);
        assert_eq!(
            state(&pool, discovered).await,
            (
                AnalysisStatus::Discovered,
                None,
                Some("/t/c.webp".to_string())
            )
        );
        assert_eq!(status(&pool, analyzing).await, AnalysisStatus::Analyzing);
    }

    #[sqlx::test(migrations = "../../infra/migrations")]
    async fn reset_covers_reopens_complete_books_in_scope(pool: PgPool) {
        let lib = library(&pool).await;
        let other = library(&pool).await;
        let complete = book(
            &pool,
            lib,
            AnalysisStatus::Complete,
            Some(4),
            Some("/t/d.webp"),
        )
        .await;
        let failed = book(&pool, lib, AnalysisStatus::Failed, None, Some("/t/e.webp")).await;
        let elsewhere = book(
            &pool,
            other,
            AnalysisStatus::Complete,
            Some(4),
            Some("/t/f.webp"),
        )
        .await;

        let reset = reset_covers(&pool, Some(lib)).await.unwrap();

        assert_eq!(reset, vec![complete]);
        assert_eq!(
            state(&pool, complete).await,
            (AnalysisStatus::CoverPending, Some(4), None)
        );
        assert_eq!(
            state(&pool, failed).await,
            (AnalysisStatus::Failed, None, Some("/t/e.webp".to_string()))
        );
        assert_eq!(status(&pool, elsewhere).await, AnalysisStatus::Complete);
    }
}
