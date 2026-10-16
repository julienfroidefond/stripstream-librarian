-- Restore the raw trigram indexes (0105 / 0119) and drop the normalized ones.
CREATE INDEX IF NOT EXISTS idx_books_title_trgm
    ON books USING gin (title gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_books_author_trgm
    ON books USING gin (author gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_series_name_trgm
    ON series USING gin (name gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_books_authors_text_trgm
    ON books USING gin (authors_search_text(authors, author) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_series_authors_text_trgm
    ON series USING gin (authors_search_text(authors, NULL::text) gin_trgm_ops);

DROP INDEX IF EXISTS idx_series_norm_authors_trgm;
DROP INDEX IF EXISTS idx_books_norm_authors_trgm;
DROP INDEX IF EXISTS idx_books_norm_author_trgm;
DROP INDEX IF EXISTS idx_books_norm_title_trgm;
