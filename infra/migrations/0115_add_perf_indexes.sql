-- Performance indexes identified during the SQL audit.
-- Note: no CONCURRENTLY — sqlx runs migrations in a transaction.

-- index_jobs: scheduler/watcher correlate on (library_id, status); PostgreSQL
-- does not index FK columns automatically.
CREATE INDEX IF NOT EXISTS idx_index_jobs_library_status
    ON index_jobs(library_id, status);

-- books: analyzer scans pending page_count / thumbnail_path per library.
CREATE INDEX IF NOT EXISTS idx_books_pending_analysis
    ON books(library_id) WHERE page_count IS NULL;

CREATE INDEX IF NOT EXISTS idx_books_pending_thumbnail
    ON books(library_id) WHERE thumbnail_path IS NULL;

-- books: rematch_unlinked_books joins on (series_id, volume).
CREATE INDEX IF NOT EXISTS idx_books_series_id_volume
    ON books(series_id, volume);

-- external_book_metadata: rematch_unlinked_books looks up unlinked rows by volume.
CREATE INDEX IF NOT EXISTS idx_ebm_link_unlinked_volume
    ON external_book_metadata(link_id, volume_number) WHERE book_id IS NULL;

-- series: array overlap (&&) on publishers in related/recommendations.
CREATE INDEX IF NOT EXISTS idx_series_publishers_gin
    ON series USING GIN (publishers);

-- book_files: stats use DISTINCT ON (book_id) ORDER BY book_id, updated_at DESC.
CREATE INDEX IF NOT EXISTS idx_book_files_book_updated
    ON book_files(book_id, updated_at DESC);

-- external_metadata_links: books list filters series_id + library_id + status,
-- ordered by created_at DESC.
CREATE INDEX IF NOT EXISTS idx_eml_series_library_status
    ON external_metadata_links(series_id, library_id, status, created_at DESC);

-- unaccent() is STABLE, so it cannot be used directly in an index expression.
-- This IMMUTABLE wrapper makes normalized-name indexes possible.
CREATE OR REPLACE FUNCTION norm_text(text)
RETURNS text LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT AS
$$ SELECT lower(public.unaccent('public.unaccent', $1)) $$;

-- series: get_or_create_series / restore_archived_data match on normalized name.
CREATE INDEX IF NOT EXISTS idx_series_library_norm_name
    ON series(library_id, norm_text(name));

CREATE INDEX IF NOT EXISTS idx_series_library_norm_original
    ON series(library_id, norm_text(original_name))
    WHERE original_name IS NOT NULL;

-- archived_series: restore_archived_data matches on normalized name.
CREATE INDEX IF NOT EXISTS idx_archived_series_library_norm_name
    ON archived_series(library_id, norm_text(name));
