-- =============================================================================
-- Migration 0134 (down): remove the 'partial' job status
-- =============================================================================
--
-- The old constraint cannot represent `partial`, so such jobs fall back to
-- `failed` (the closest status: something did fail). A diagnostic message is
-- only added when the job did not already carry an error.

UPDATE index_jobs
SET status = 'failed',
    error_opt = COALESCE(error_opt, 'partial status removed by migration 0134 down')
WHERE status = 'partial';

ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_status_check,
  ADD CONSTRAINT index_jobs_status_check
    CHECK (status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails', 'success', 'failed', 'cancelled'));
