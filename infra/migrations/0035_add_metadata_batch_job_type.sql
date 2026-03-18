-- Allow metadata_batch job type in index_jobs
ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN ('scan', 'rebuild', 'full_rebuild', 'thumbnail_rebuild', 'thumbnail_regenerate', 'cbr_to_cbz', 'metadata_batch'));
