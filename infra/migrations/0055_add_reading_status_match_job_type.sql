-- Add reading_status_match job type: auto-matches library series against the
-- configured reading status provider (e.g. AniList) and creates links.
ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN ('scan', 'rebuild', 'full_rebuild', 'rescan', 'thumbnail_rebuild', 'thumbnail_regenerate', 'cbr_to_cbz', 'metadata_batch', 'metadata_refresh', 'reading_status_match'));
