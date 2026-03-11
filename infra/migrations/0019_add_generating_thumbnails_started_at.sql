-- Add timestamp for Phase 2b (generating_thumbnails) so we can show separate durations:
-- Phase 2a: phase2_started_at → generating_thumbnails_started_at (extracting_pages)
-- Phase 2b: generating_thumbnails_started_at → finished_at
ALTER TABLE index_jobs
  ADD COLUMN IF NOT EXISTS generating_thumbnails_started_at TIMESTAMPTZ;
