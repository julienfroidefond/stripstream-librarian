DROP INDEX IF EXISTS idx_series_authors_text_trgm;
DROP INDEX IF EXISTS idx_books_authors_text_trgm;
DROP FUNCTION IF EXISTS authors_search_text(text[], text);
