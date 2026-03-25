-- Table to store per-series results for reading_status_match jobs
CREATE TABLE reading_status_match_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES index_jobs(id) ON DELETE CASCADE,
    library_id UUID NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    series_name TEXT NOT NULL,
    status TEXT NOT NULL,  -- 'linked', 'already_linked', 'no_results', 'ambiguous', 'error'
    anilist_id INTEGER,
    anilist_title TEXT,
    anilist_url TEXT,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_rsmr_job_id ON reading_status_match_results(job_id);
CREATE INDEX idx_rsmr_status ON reading_status_match_results(status);
