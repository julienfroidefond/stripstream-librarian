-- Trigram indexes backing the /search author branches.
--
-- The search query matches authors on the concatenated text form
-- (`authors_search_text(authors, author) ILIKE $1`) so the GIN trigram index
-- can accelerate the substring lookup without a sequential scan. The exact
-- per-element predicate is re-applied on the small candidate set afterwards,
-- so the semantics stay identical.
--
-- `array_to_string` is only STABLE, so it cannot be used directly in an index
-- expression. `authors_search_text` is the IMMUTABLE wrapper that also folds in
-- the legacy scalar `author` fallback (same trick as `norm_text` in 0115).
CREATE OR REPLACE FUNCTION authors_search_text(names text[], fallback text)
RETURNS text LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
$$
    SELECT array_to_string(
        COALESCE(
            names,
            CASE WHEN fallback IS NOT NULL AND fallback != ''
                 THEN ARRAY[fallback]
                 ELSE ARRAY[]::text[]
            END
        ),
        ' '
    )
$$;

CREATE INDEX IF NOT EXISTS idx_books_authors_text_trgm
    ON books USING gin (authors_search_text(authors, author) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_series_authors_text_trgm
    ON series USING gin (authors_search_text(authors, NULL::text) gin_trgm_ops);
