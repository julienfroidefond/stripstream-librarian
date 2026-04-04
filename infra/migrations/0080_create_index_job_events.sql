-- Unified event tracking for all job types.
-- Replaces per-job-type results tables over time.
CREATE TABLE index_job_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES index_jobs(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    level TEXT NOT NULL DEFAULT 'info',
    entity_type TEXT,
    entity_id UUID,
    entity_name TEXT,
    message TEXT,
    detail JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ije_job_id ON index_job_events(job_id);
CREATE INDEX idx_ije_event_type ON index_job_events(event_type);
CREATE INDEX idx_ije_level ON index_job_events(level) WHERE level != 'info';
CREATE INDEX idx_ije_entity ON index_job_events(entity_type, entity_id) WHERE entity_id IS NOT NULL;
