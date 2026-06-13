-- Performance indexes identified during review
-- Note: no CONCURRENTLY — sqlx runs migrations in a transaction

CREATE INDEX IF NOT EXISTS idx_eml_series_status
    ON external_metadata_links(series_id, status);

CREATE INDEX IF NOT EXISTS idx_anilist_links_series_id
    ON anilist_series_links(series_id);

CREATE INDEX IF NOT EXISTS idx_books_created_at_desc
    ON books(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_series_user_ratings_user_series
    ON series_user_ratings(user_id, series_id);

CREATE INDEX IF NOT EXISTS idx_reading_list_items_list_series
    ON reading_list_items(list_id, series_id);
