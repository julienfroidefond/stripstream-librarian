-- =============================================================================
-- Migration 0127: store the torrent name on torrent_downloads
-- =============================================================================
--
-- The admin Downloads page shows the release/file name under each row, like the
-- Telegram and DDL-provider rows already do (`filename`). Torrent rows only had
-- a hash, so the name is captured from AllDebrid (the upload response `name`,
-- kept fresh by the magnet status `filename`). It stays NULL for qBittorrent
-- rows for now.

ALTER TABLE torrent_downloads
    ADD COLUMN name TEXT;
