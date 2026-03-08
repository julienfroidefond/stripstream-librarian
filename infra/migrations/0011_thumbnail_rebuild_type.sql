-- Migration: Add job type 'thumbnail_rebuild' for manual thumbnail generation

ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN ('scan', 'rebuild', 'full_rebuild', 'thumbnail_rebuild', 'thumbnail_regenerate'));
