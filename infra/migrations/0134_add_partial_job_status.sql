-- =============================================================================
-- Migration 0134: add the 'partial' terminal job status
-- =============================================================================
--
-- A job is `partial` when at least one unit of work failed but others
-- succeeded. It is additive: `success`, `failed`, `cancelled` and the
-- thumbnail sub-statuses keep their exact meaning.

ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_status_check,
  ADD CONSTRAINT index_jobs_status_check
    CHECK (status IN ('pending', 'running', 'extracting_pages', 'generating_thumbnails', 'success', 'partial', 'failed', 'cancelled'));
