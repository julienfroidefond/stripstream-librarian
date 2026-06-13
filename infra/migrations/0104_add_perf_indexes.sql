-- Performance indexes identified during review

-- Speeds up /series/:id/ratings provider lookup (series_id filter on EML)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_eml_series_status
    ON external_metadata_links(series_id, status);

-- Speeds up /series/:id/ratings AniList score lookup
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_anilist_links_series_id
    ON anilist_series_links(series_id);

-- Speeds up books ORDER BY latest (created_at DESC sort)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_books_created_at_desc
    ON books(created_at DESC);

-- Speeds up series_user_ratings lookup by user (rated_only filter + ratings endpoint)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_series_user_ratings_user_series
    ON series_user_ratings(user_id, series_id);

-- Speeds up reading_list_items self-join in recommendations
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reading_list_items_list_series
    ON reading_list_items(list_id, series_id);
