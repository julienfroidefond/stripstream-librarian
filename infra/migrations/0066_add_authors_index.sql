-- GIN index for efficient array lookups on books.authors
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_books_authors_gin ON books USING GIN (authors);

-- Index on author column for legacy single-author field
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_books_author ON books (author) WHERE author IS NOT NULL AND author != '';

-- GIN index on series_metadata.authors
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_series_metadata_authors_gin ON series_metadata USING GIN (authors);
