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

REINDEX INDEX idx_books_norm_authors_trgm;
REINDEX INDEX idx_series_norm_authors_trgm;
