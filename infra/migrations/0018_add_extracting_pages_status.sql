-- Migration: Add status 'extracting_pages' for the first sub-phase of thumbnail generation
-- Phase 1 (extracting_pages): extract raw first-page image from archive, store as-is
-- Phase 2 (generating_thumbnails): resize and encode as WebP
ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_status_check,
  ADD CONSTRAINT index_jobs_status_check
    CHECK (status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails', 'success', 'failed', 'cancelled'));
