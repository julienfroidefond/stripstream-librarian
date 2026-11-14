-- =============================================================================
-- Migration 0142: remember whether Stripstream added the qBittorrent torrent
-- =============================================================================
--
-- A managed download whose hash already exists in qBittorrent (a torrent the
-- user added by hand) is adopted instead of added. Cleaning up such a row must
-- never delete the user's torrent and its files from qBittorrent:
-- `qb_owned = false` marks it. Every existing row was added by Stripstream.

ALTER TABLE torrent_downloads
    ADD COLUMN qb_owned BOOLEAN NOT NULL DEFAULT TRUE;
