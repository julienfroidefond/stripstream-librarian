-- Revert: drop the ddl_search job type from index_jobs.
-- Existing rows keep the type, so they must go before the narrower CHECK is
-- re-validated (otherwise the ALTER fails on a non-empty history).
DELETE FROM index_jobs WHERE type = 'ddl_search';

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
