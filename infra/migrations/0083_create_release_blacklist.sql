CREATE TABLE release_blacklist (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    indexer TEXT,
    series_name TEXT,
    blacklisted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (title)
);

CREATE INDEX idx_release_blacklist_title ON release_blacklist(title);
