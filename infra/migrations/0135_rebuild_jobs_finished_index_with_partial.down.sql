-- =============================================================================
-- Migration 0135 (down): restore the finished-jobs index without 'partial'
-- =============================================================================
--
-- Mirrors the predicate created by migration 0118.

DROP INDEX IF EXISTS idx_index_jobs_finished;

CREATE INDEX IF NOT EXISTS idx_index_jobs_finished
    ON index_jobs (finished_at)
    WHERE status IN ('success', 'failed');
