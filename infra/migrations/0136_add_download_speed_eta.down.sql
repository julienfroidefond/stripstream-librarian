-- =============================================================================
-- Migration 0136 (down): drop the transfer rate + ETA columns
-- =============================================================================

ALTER TABLE telegram_book_links
    DROP COLUMN IF EXISTS download_speed,
    DROP COLUMN IF EXISTS eta;

ALTER TABLE ddl_provider_downloads
    DROP COLUMN IF EXISTS download_speed,
    DROP COLUMN IF EXISTS eta;
