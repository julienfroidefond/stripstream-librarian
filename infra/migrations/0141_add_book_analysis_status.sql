-- Make the book analysis lifecycle explicit instead of encoding it through
-- nullable page_count / thumbnail_path.
ALTER TABLE books
    ADD COLUMN analysis_status TEXT NOT NULL DEFAULT 'discovered';

ALTER TABLE books
    ADD CONSTRAINT books_analysis_status_check
    CHECK (analysis_status IN ('discovered', 'analyzing', 'cover_pending', 'complete', 'failed'));

-- Backfill existing rows from the state they already encode. A parse error with
-- no page count means the last analysis attempt failed; everything else with no
-- page count is still waiting for its first successful analysis.
UPDATE books
SET analysis_status = CASE
    WHEN page_count IS NOT NULL AND thumbnail_path IS NOT NULL THEN 'complete'
    WHEN page_count IS NOT NULL THEN 'cover_pending'
    WHEN EXISTS (
        SELECT 1
        FROM book_files bf
        WHERE bf.book_id = books.id
          AND bf.parse_status = 'error'
    ) THEN 'failed'
    ELSE 'discovered'
END;

-- Replace the nullable-column partial indexes with indexes on the state the
-- analyzer actually selects.
DROP INDEX IF EXISTS idx_books_pending_analysis;
DROP INDEX IF EXISTS idx_books_pending_thumbnail;

CREATE INDEX idx_books_analysis_pending
    ON books(library_id)
    WHERE analysis_status IN ('discovered', 'analyzing', 'failed');

CREATE INDEX idx_books_analysis_thumbnail
    ON books(library_id)
    WHERE analysis_status = 'cover_pending';
