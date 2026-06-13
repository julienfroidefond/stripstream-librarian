DROP INDEX IF EXISTS idx_series_user_ratings_series;
DROP TABLE IF EXISTS series_user_ratings;

ALTER TABLE anilist_series_links DROP COLUMN IF EXISTS user_score;

ALTER TABLE external_metadata_links
    DROP COLUMN IF EXISTS provider_rating,
    DROP COLUMN IF EXISTS provider_rating_count,
    DROP COLUMN IF EXISTS provider_rating_scale;
