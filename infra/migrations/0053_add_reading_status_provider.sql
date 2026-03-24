-- Replace anilist_enabled boolean with generic reading_status_provider
ALTER TABLE libraries ADD COLUMN reading_status_provider TEXT;
UPDATE libraries SET reading_status_provider = 'anilist' WHERE anilist_enabled = TRUE;
ALTER TABLE libraries DROP COLUMN anilist_enabled;

-- Add provider column to anilist_series_links for future multi-provider support
ALTER TABLE anilist_series_links ADD COLUMN provider TEXT NOT NULL DEFAULT 'anilist';
-- Update the primary key to include provider
ALTER TABLE anilist_series_links DROP CONSTRAINT anilist_series_links_pkey;
ALTER TABLE anilist_series_links ADD PRIMARY KEY (library_id, series_name, provider);
