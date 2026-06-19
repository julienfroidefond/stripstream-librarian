-- Preserve AniList link data in archived_series so DROPPED can be pushed
-- after a series is deleted from disk (anilist_series_links cascades on DELETE).
ALTER TABLE archived_series
    ADD COLUMN IF NOT EXISTS anilist_id INTEGER,
    ADD COLUMN IF NOT EXISTS anilist_title TEXT,
    ADD COLUMN IF NOT EXISTS anilist_url TEXT,
    ADD COLUMN IF NOT EXISTS anilist_dropped_pushed_at TIMESTAMPTZ;
