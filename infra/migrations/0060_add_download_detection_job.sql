ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN ('scan', 'rebuild', 'full_rebuild', 'rescan', 'thumbnail_rebuild', 'thumbnail_regenerate', 'cbr_to_cbz', 'metadata_batch', 'metadata_refresh', 'reading_status_match', 'reading_status_push', 'download_detection'));

CREATE TABLE download_detection_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES index_jobs(id) ON DELETE CASCADE,
    library_id UUID NOT NULL,
    series_name TEXT NOT NULL,
    -- 'found' | 'not_found' | 'no_missing' | 'no_metadata' | 'error'
    status TEXT NOT NULL,
    missing_count INTEGER NOT NULL DEFAULT 0,
    -- JSON array of available Prowlarr releases (simplified)
    available_releases JSONB,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX download_detection_results_job_id_idx ON download_detection_results(job_id);
