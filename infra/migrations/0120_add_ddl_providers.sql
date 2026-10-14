-- =============================================================================
-- Migration 0120: DDL providers — direct HTTP file downloads
-- =============================================================================
--
-- A "DDL provider" is an external catalogue (JSON API or scraped HTML) that
-- can be searched for a series/tome and streamed straight into a library. Unlike
-- torrents (qBittorrent) or Telegram media, the file is fetched over plain HTTP
-- and imported into the library directory, then picked up by a scan job.

CREATE TABLE ddl_provider_downloads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- DDL provider registry id (see downloads::ddl_provider::registry)
    provider TEXT NOT NULL,
    -- DDL-provider-specific file identifier (fourtoutici: file_id)
    external_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    file_size BIGINT,
    bytes_downloaded BIGINT NOT NULL DEFAULT 0,
    series_name TEXT,
    volume_number INTEGER,
    library_id UUID REFERENCES libraries(id) ON DELETE SET NULL,
    -- queued | downloading | imported | failed | dismissed
    status TEXT NOT NULL DEFAULT 'queued',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider, external_id)
);

CREATE INDEX idx_ddl_provider_downloads_status ON ddl_provider_downloads (status);
CREATE INDEX idx_ddl_provider_downloads_library_series
    ON ddl_provider_downloads (library_id, series_name);

-- Seed the DDL provider registry. `enabled` is per-provider (off by default, like the
-- other download features); `base_url` allows self-hosting/mirroring without a
-- redeploy.
INSERT INTO app_settings (key, value) VALUES
    ('ddl_providers', '{"providers": {"fourtoutici": {"enabled": false, "base_url": "https://fourtoutici.cc"}}}')
ON CONFLICT DO NOTHING;
