DROP INDEX IF EXISTS idx_books_analysis_thumbnail;
DROP INDEX IF EXISTS idx_books_analysis_pending;

ALTER TABLE books DROP CONSTRAINT IF EXISTS books_analysis_status_check;
ALTER TABLE books DROP COLUMN IF EXISTS analysis_status;

CREATE INDEX IF NOT EXISTS idx_books_pending_analysis
    ON books(library_id)
    WHERE page_count IS NULL;

CREATE INDEX IF NOT EXISTS idx_books_pending_thumbnail
    ON books(library_id)
    WHERE thumbnail_path IS NULL;
