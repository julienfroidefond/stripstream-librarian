-- Improve recommendations query performance

-- book_reading_progress: user_id first for user-scoped lookups
-- (existing index is (status, user_id) which is wrong order for WHERE user_id = $1)
CREATE INDEX IF NOT EXISTS idx_book_reading_progress_user_id
    ON book_reading_progress(user_id, status);

-- GIN index on series.authors for array overlap (&&) in recommendations
CREATE INDEX IF NOT EXISTS idx_series_authors_gin
    ON series USING GIN (authors);

-- reading_list_items: lookup by series_id for co-occurrence pre-computation
CREATE INDEX IF NOT EXISTS idx_reading_list_items_series_id
    ON reading_list_items(series_id, list_id);
