-- Add rescan job type: clears directory mtimes to force re-walking all directories
-- while preserving existing data (unlike full_rebuild which deletes everything).
-- Useful for discovering newly supported formats (e.g. EPUB) without losing metadata.
ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN ('scan', 'rebuild', 'full_rebuild', 'rescan', 'thumbnail_rebuild', 'thumbnail_regenerate', 'cbr_to_cbz', 'metadata_batch', 'metadata_refresh'));
