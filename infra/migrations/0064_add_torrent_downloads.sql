ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN (
      'scan', 'rebuild', 'full_rebuild', 'rescan',
      'thumbnail_rebuild', 'thumbnail_regenerate',
      'cbr_to_cbz',
      'metadata_batch', 'metadata_refresh',
      'reading_status_match', 'reading_status_push',
      'download_detection',
      'torrent_import'
    ));

CREATE TABLE torrent_downloads (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id       UUID        NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    series_name      TEXT        NOT NULL,
    expected_volumes INTEGER[]   NOT NULL DEFAULT '{}',
    qb_hash          TEXT,
    content_path     TEXT,
    status           TEXT        NOT NULL DEFAULT 'downloading'
                     CHECK (status IN ('downloading', 'completed', 'importing', 'imported', 'error')),
    imported_files   JSONB,
    error_message    TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX torrent_downloads_status_idx  ON torrent_downloads(status);
CREATE INDEX torrent_downloads_qb_hash_idx ON torrent_downloads(qb_hash) WHERE qb_hash IS NOT NULL;
