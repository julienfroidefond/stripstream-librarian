-- Trigram index backing accent- and case-insensitive series name search.
-- `norm_text` (lower + unaccent) is defined in 0115 and is IMMUTABLE, so it can
-- be used as an index expression. Queries match with
-- `norm_text(s.name) LIKE norm_text($1)`, which lets the GIN trigram index
-- accelerate the pattern lookup on the normalized expression.
-- `pg_trgm` is installed in 0105.
CREATE INDEX IF NOT EXISTS idx_series_norm_name_trgm
    ON series USING gin(norm_text(name) gin_trgm_ops);
