ALTER TABLE torrent_downloads
  ADD COLUMN replace_existing BOOLEAN NOT NULL DEFAULT FALSE;
