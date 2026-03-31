-- Allow metadata_refresh_all job type in index_jobs
ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN (
      'scan', 'rebuild', 'full_rebuild', 'rescan',
      'thumbnail_rebuild', 'thumbnail_regenerate',
      'cbr_to_cbz',
      'metadata_batch', 'metadata_refresh', 'metadata_refresh_all',
      'reading_status_match', 'reading_status_push',
      'download_detection',
      'torrent_import'
    ));
