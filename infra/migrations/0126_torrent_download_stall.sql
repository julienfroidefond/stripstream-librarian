-- =============================================================================
-- Migration 0126: expose seeders and detect stalled AllDebrid magnets
-- =============================================================================
--
-- AllDebrid reports how many peers are seeding a magnet. A torrent with no peer
-- never downloads a single byte and would otherwise sit in `downloading`
-- forever. `seeders` is surfaced to the admin UI, and `stall_started_at`
-- records when the transfer was first observed at zero bytes so the poller can
-- give up after a while (a manual restart clears the marker).

ALTER TABLE torrent_downloads
    ADD COLUMN seeders INTEGER,
    ADD COLUMN stall_started_at TIMESTAMPTZ;
