-- =============================================================================
-- Migration 0136: transfer rate + ETA on Telegram and DDL downloads
-- =============================================================================
--
-- The download history already shows a speed and an ETA for torrents
-- (`torrent_downloads`, migration 0065). Telegram media and direct provider
-- downloads only stored a byte count, so their rows could not display the same
-- information. Both are transferred by the ranged engine, which reports the
-- written bytes once per second: the API turns those samples into a smoothed
-- rate and a remaining time.
--
-- Units match `torrent_downloads`: `download_speed` is bytes per second, `eta`
-- is seconds.

ALTER TABLE telegram_book_links
    ADD COLUMN download_speed BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN eta            BIGINT NOT NULL DEFAULT 0;

ALTER TABLE ddl_provider_downloads
    ADD COLUMN download_speed BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN eta            BIGINT NOT NULL DEFAULT 0;
