-- Trigram indexes for ILIKE text search on books
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS idx_books_title_trgm
    ON books USING gin(title gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_books_author_trgm
    ON books USING gin(author gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_series_name_trgm
    ON series USING gin(name gin_trgm_ops);
