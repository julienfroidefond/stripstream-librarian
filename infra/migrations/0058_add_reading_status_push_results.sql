CREATE TABLE reading_status_push_results (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES index_jobs(id) ON DELETE CASCADE,
    library_id UUID NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    series_name TEXT NOT NULL,
    -- 'pushed' | 'skipped' | 'no_books' | 'error'
    status TEXT NOT NULL,
    anilist_id INTEGER,
    anilist_title TEXT,
    anilist_url TEXT,
    -- what was actually sent to AniList (NULL when skipped/error)
    anilist_status TEXT,
    progress_volumes INTEGER,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_rspr_job_id ON reading_status_push_results(job_id);
CREATE INDEX idx_rspr_status ON reading_status_push_results(status);
