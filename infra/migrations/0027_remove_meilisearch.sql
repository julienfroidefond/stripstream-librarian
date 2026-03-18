-- Remove Meilisearch sync tracking (search is now handled by PostgreSQL)
DROP TABLE IF EXISTS sync_metadata;

-- Enable pg_trgm for fuzzy search
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Add trigram indexes for search performance
CREATE INDEX IF NOT EXISTS idx_books_title_trgm ON books USING gin (title gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_books_series_trgm ON books USING gin (series gin_trgm_ops);
