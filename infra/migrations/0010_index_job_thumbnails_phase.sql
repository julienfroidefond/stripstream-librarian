-- Migration: Add status 'generating_thumbnails' for thumbnail phase after indexing

ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_status_check,
  ADD CONSTRAINT index_jobs_status_check
    CHECK (status IN ('pending', 'running', 'generating_thumbnails', 'success', 'failed'));
