-- =============================================================================
-- Migration 0125: AllDebrid as an alternative download client
-- =============================================================================
--
-- A Prowlarr snatch used to always go to qBittorrent. AllDebrid is a debrid
-- service that downloads the torrent on its servers and exposes the resulting
-- files as direct HTTP links, so no local BitTorrent client, VPN or seeding is
-- needed. The `torrent_downloads` table stays the single ledger for both
-- clients: `client` tells the poller/import/webhook which backend a row belongs
-- to, and `external_id` stores the backend-specific identifier (the AllDebrid
-- magnet id; unused for qBittorrent, which matches on `qb_hash`).

ALTER TABLE torrent_downloads
    ADD COLUMN client TEXT NOT NULL DEFAULT 'qbittorrent',
    -- AllDebrid magnet id (TEXT: the API returns it as an integer, but the
    -- column is opaque and may hold other backends' ids later).
    ADD COLUMN external_id TEXT;

ALTER TABLE torrent_downloads
    DROP CONSTRAINT IF EXISTS torrent_downloads_client_check,
    ADD CONSTRAINT torrent_downloads_client_check
        CHECK (client IN ('qbittorrent', 'alldebrid'));

-- The AllDebrid poller and the qBittorrent poller each scan their own subset.
CREATE INDEX torrent_downloads_client_status_idx
    ON torrent_downloads (client, status);

-- Seed the AllDebrid integration. Disabled (opt-in) like the other download
-- features; the API key is stored here and never echoed back by the settings
-- endpoints (see `settings::redact_secrets`).
INSERT INTO app_settings (key, value) VALUES
    ('alldebrid', '{"api_key": "", "enabled": false}')
ON CONFLICT DO NOTHING;
