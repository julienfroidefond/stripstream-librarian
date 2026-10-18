-- Reverting drops the AllDebrid client tracking. Rows created with
-- client='alldebrid' would lose their backend id; they are removed first so no
-- orphan row is left pointing at a backend the poller no longer knows about.
DELETE FROM torrent_downloads WHERE client = 'alldebrid';

DROP INDEX IF EXISTS torrent_downloads_client_status_idx;

ALTER TABLE torrent_downloads
    DROP CONSTRAINT IF EXISTS torrent_downloads_client_check,
    DROP COLUMN IF EXISTS external_id,
    DROP COLUMN IF EXISTS client;

DELETE FROM app_settings WHERE key = 'alldebrid';
