-- `authors_search_text` (0119) fell back to the legacy scalar `author` only
-- when `authors` was NULL, but `books.authors` is `NOT NULL DEFAULT '{}'`: a
-- book whose authors only live in the legacy column was never found by the
-- /search author branch. Treat an empty array like NULL, as `/books` does
-- (`COALESCE(NULLIF(b.authors, '{}'), ...)`).
--
-- The function backs expression indexes (0123), whose stored keys were computed
-- with the old body: rebuild them after replacing it.
CREATE OR REPLACE FUNCTION authors_search_text(names text[], fallback text)
RETURNS text LANGUAGE sql IMMUTABLE PARALLEL SAFE AS
$$
    SELECT array_to_string(
        COALESCE(
            NULLIF(names, ARRAY[]::text[]),
            CASE WHEN fallback IS NOT NULL AND fallback != ''
                 THEN ARRAY[fallback]
                 ELSE ARRAY[]::text[]
            END
        ),
        ' '
    )
$$;

REINDEX INDEX idx_books_norm_authors_trgm;
REINDEX INDEX idx_series_norm_authors_trgm;
