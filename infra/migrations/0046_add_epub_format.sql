-- Add EPUB to allowed format values in book_files and books tables.
-- PostgreSQL CHECK constraints are dropped+recreated (no ALTER CONSTRAINT).

-- book_files.format
ALTER TABLE book_files DROP CONSTRAINT IF EXISTS book_files_format_check;
ALTER TABLE book_files ADD CONSTRAINT book_files_format_check CHECK (format IN ('pdf', 'cbz', 'cbr', 'epub'));

-- books.format (denormalized column added in 0020)
ALTER TABLE books DROP CONSTRAINT IF EXISTS books_format_check;
ALTER TABLE books ADD CONSTRAINT books_format_check CHECK (format IN ('pdf', 'cbz', 'cbr', 'epub'));
