ALTER TABLE torrent_downloads
  DROP CONSTRAINT IF EXISTS torrent_downloads_status_check,
  ADD CONSTRAINT torrent_downloads_status_check
    CHECK (status IN ('downloading', 'completed', 'importing', 'imported', 'partial', 'no_files_imported', 'error'));
