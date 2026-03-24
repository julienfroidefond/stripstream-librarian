-- Add AniList sync support
ALTER TABLE libraries ADD COLUMN anilist_enabled BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE anilist_series_links (
    library_id    UUID    NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    series_name   TEXT    NOT NULL,
    anilist_id    INTEGER NOT NULL,
    anilist_title TEXT,
    anilist_url   TEXT,
    status        TEXT    NOT NULL DEFAULT 'linked', -- 'linked' | 'synced' | 'error'
    linked_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    synced_at     TIMESTAMPTZ,
    PRIMARY KEY (library_id, series_name)
);

CREATE INDEX idx_anilist_series_links_library ON anilist_series_links(library_id);
