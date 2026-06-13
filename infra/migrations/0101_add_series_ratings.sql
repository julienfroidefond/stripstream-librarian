-- Provider ratings stored alongside external metadata links
ALTER TABLE external_metadata_links
    ADD COLUMN provider_rating       REAL,
    ADD COLUMN provider_rating_count INTEGER,
    ADD COLUMN provider_rating_scale REAL;

-- AniList user personal score (normalised to 0-10, NULL if not rated)
ALTER TABLE anilist_series_links
    ADD COLUMN user_score REAL;

-- Per-user series ratings (half-star scale: 1-10, stored as integer halves)
CREATE TABLE series_user_ratings (
    user_id    UUID        NOT NULL REFERENCES users(id)   ON DELETE CASCADE,
    series_id  UUID        NOT NULL REFERENCES series(id)  ON DELETE CASCADE,
    rating     SMALLINT    NOT NULL CHECK (rating BETWEEN 1 AND 10),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, series_id)
);

CREATE INDEX idx_series_user_ratings_series ON series_user_ratings(series_id);
