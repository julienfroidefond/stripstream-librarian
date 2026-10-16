-- Trigram indexes backing accent- and case-insensitive text search (`/search`,
-- `/books`, `/authors`, `/series`, `/genres`).
--
-- The queries match with `norm_text(col) LIKE norm_text($1)`. `norm_text`
-- (lower + unaccent) is IMMUTABLE (0115) and `authors_search_text` is IMMUTABLE
-- (0119), so both can be used as index expressions. `idx_series_norm_name_trgm`
-- already covers the series-name branch (0116); this migration adds the missing
-- normalized indexes for book titles, book authors and series authors.
--
-- Definition order first, then the superseded raw indexes are dropped: every
-- search path now normalizes both sides, so `title`/`author`/`name` and the raw
-- `authors_search_text` indexes can no longer be used. Keeping them would only
-- add GIN write amplification to every book/series insert (indexer phase 1/2).
CREATE INDEX IF NOT EXISTS idx_books_norm_title_trgm
    ON books USING gin (norm_text(title) gin_trgm_ops);

-- `/books` also filters on the legacy scalar `author` column.
CREATE INDEX IF NOT EXISTS idx_books_norm_author_trgm
    ON books USING gin (norm_text(author) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_books_norm_authors_trgm
    ON books USING gin (norm_text(authors_search_text(authors, author)) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_series_norm_authors_trgm
    ON series USING gin (norm_text(authors_search_text(authors, NULL::text)) gin_trgm_ops);

-- Superseded by the normalized indexes above.
DROP INDEX IF EXISTS idx_books_title_trgm;
DROP INDEX IF EXISTS idx_books_author_trgm;
DROP INDEX IF EXISTS idx_books_authors_text_trgm;
DROP INDEX IF EXISTS idx_series_name_trgm;
DROP INDEX IF EXISTS idx_series_authors_text_trgm;
