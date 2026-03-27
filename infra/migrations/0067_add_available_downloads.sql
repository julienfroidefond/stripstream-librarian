CREATE TABLE available_downloads (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id       UUID        NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    series_name      TEXT        NOT NULL,
    missing_count    INTEGER     NOT NULL DEFAULT 0,
    available_releases JSONB,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(library_id, series_name)
);

CREATE INDEX idx_available_downloads_library ON available_downloads(library_id);

-- Migrate existing detection results into the new table
INSERT INTO available_downloads (library_id, series_name, missing_count, available_releases, updated_at)
SELECT DISTINCT ON (library_id, series_name)
    library_id, series_name, missing_count, available_releases, created_at
FROM download_detection_results
WHERE status = 'found' AND available_releases IS NOT NULL
ORDER BY library_id, series_name, created_at DESC
ON CONFLICT DO NOTHING;
