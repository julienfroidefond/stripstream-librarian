ALTER TABLE torrent_downloads
    DROP COLUMN IF EXISTS stall_started_at,
    DROP COLUMN IF EXISTS seeders;
