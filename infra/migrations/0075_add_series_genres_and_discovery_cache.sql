ALTER TABLE series ADD COLUMN genres TEXT[] NOT NULL DEFAULT '{}';
CREATE INDEX idx_series_genres ON series USING GIN (genres);

CREATE TABLE discovery_cache (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cache_key TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL,
    query_type TEXT NOT NULL,
    results JSONB NOT NULL DEFAULT '[]',
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_discovery_cache_key ON discovery_cache(cache_key);
CREATE INDEX idx_discovery_cache_expires ON discovery_cache(expires_at);
