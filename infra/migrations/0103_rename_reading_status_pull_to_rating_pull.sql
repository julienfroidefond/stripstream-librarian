-- Rename job type: reading_status_pull → rating_pull (scores, not reading status)
ALTER TABLE index_jobs
  DROP CONSTRAINT IF EXISTS index_jobs_type_check,
  ADD CONSTRAINT index_jobs_type_check
    CHECK (type IN (
      'scan', 'rebuild', 'full_rebuild', 'rescan',
      'thumbnail_rebuild', 'thumbnail_regenerate',
      'cbr_to_cbz',
      'metadata_batch', 'metadata_batch_rematch',
      'metadata_refresh', 'metadata_refresh_all',
      'reading_status_match', 'reading_status_push',
      'rating_pull',
      'download_detection',
      'torrent_import',
      'prowlarr_rss',
      'telegram_sync',
      'telegram_sync_incremental'
    ));
