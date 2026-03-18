-- Add fallback_metadata_provider to libraries
ALTER TABLE libraries ADD COLUMN fallback_metadata_provider TEXT;

-- Table to store batch metadata job results (one row per series per job)
CREATE TABLE metadata_batch_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES index_jobs(id) ON DELETE CASCADE,
    library_id UUID NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    series_name TEXT NOT NULL,
    status TEXT NOT NULL,  -- 'auto_matched', 'pending_review', 'no_results', 'too_many_results', 'low_confidence', 'already_linked', 'error'
    provider_used TEXT,
    fallback_used BOOLEAN NOT NULL DEFAULT FALSE,
    candidates_count INTEGER NOT NULL DEFAULT 0,
    best_confidence REAL,
    best_candidate_json JSONB,
    link_id UUID REFERENCES external_metadata_links(id) ON DELETE SET NULL,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mbr_job_id ON metadata_batch_results(job_id);
CREATE INDEX idx_mbr_status ON metadata_batch_results(status);
