-- =============================================================================
-- Migration 0135: include 'partial' in the finished-jobs partial index
-- =============================================================================
--
-- Migration 0118 created `idx_index_jobs_finished` with the predicate
-- `status IN ('success', 'failed')`. Migration 0134 added the `partial`
-- terminal status but left the index behind, so partial jobs were not covered
-- by the partial index used by /stats and diagnostics. Rebuild it with the
-- third terminal status.

DROP INDEX IF EXISTS idx_index_jobs_finished;

CREATE INDEX IF NOT EXISTS idx_index_jobs_finished
    ON index_jobs (finished_at)
    WHERE status IN ('success', 'partial', 'failed');
