-- Add format column to books table (denormalized from book_files for easy API access)
ALTER TABLE books ADD COLUMN IF NOT EXISTS format TEXT CHECK (format IN ('pdf', 'cbz', 'cbr'));

-- Backfill from book_files (take the format of the most recent file per book)
UPDATE books b
SET format = bf.format
FROM (
    SELECT DISTINCT ON (book_id) book_id, format
    FROM book_files
    ORDER BY book_id, updated_at DESC
) bf
WHERE b.id = bf.book_id
  AND b.format IS NULL;
