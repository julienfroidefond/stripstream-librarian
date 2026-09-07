CREATE TABLE series_user_favorites (
    user_id    UUID        NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
    series_id  UUID        NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, series_id)
);

CREATE INDEX idx_series_user_favorites_user_created
    ON series_user_favorites(user_id, created_at DESC);
