-- =============================================================================
-- Migration 0071: Drop legacy TEXT columns now replaced by series_id FK
-- Prerequisite: migration 0070 must have run and all code must use series_id
-- =============================================================================

-- 1. Drop old TEXT series column from books (replaced by series_id FK)
ALTER TABLE books DROP COLUMN IF EXISTS series;

-- 2. Drop old TEXT series_name columns from related tables
ALTER TABLE external_metadata_links DROP COLUMN IF EXISTS series_name;
ALTER TABLE anilist_series_links DROP COLUMN IF EXISTS series_name;
ALTER TABLE available_downloads DROP COLUMN IF EXISTS series_name;
ALTER TABLE download_detection_results DROP COLUMN IF EXISTS series_name;

-- 3. Drop the old series_metadata table (fused into series)
DROP TABLE IF EXISTS series_metadata;

-- 4. Drop old composite indexes that relied on series_name text matching
DROP INDEX IF EXISTS idx_eml_library_series;

-- 5. Add missing indexes on series_id columns
CREATE INDEX IF NOT EXISTS idx_eml_series_id ON external_metadata_links(series_id);
CREATE INDEX IF NOT EXISTS idx_asl_series_id ON anilist_series_links(series_id);
CREATE INDEX IF NOT EXISTS idx_ad_series_id ON available_downloads(series_id);
CREATE INDEX IF NOT EXISTS idx_ddr_series_id ON download_detection_results(series_id);

-- 6. Update unique constraints to use series_id instead of series_name
-- external_metadata_links: (library_id, series_name, provider) → (series_id, provider)
ALTER TABLE external_metadata_links DROP CONSTRAINT IF EXISTS external_metadata_links_library_id_series_name_provider_key;
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'external_metadata_links_series_id_provider_key'
    ) THEN
        ALTER TABLE external_metadata_links
            ADD CONSTRAINT external_metadata_links_series_id_provider_key UNIQUE (series_id, provider);
    END IF;
END $$;

-- anilist_series_links: old PK was (library_id, series_name, provider)
-- Need to recreate with series_id. First drop old PK, add id column, create new PK.
DO $$ BEGIN
    -- Add id column if not exists
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'anilist_series_links' AND column_name = 'id'
    ) THEN
        ALTER TABLE anilist_series_links ADD COLUMN id UUID DEFAULT gen_random_uuid();
        -- Drop old composite PK
        ALTER TABLE anilist_series_links DROP CONSTRAINT IF EXISTS anilist_series_links_pkey;
        -- Set new PK
        ALTER TABLE anilist_series_links ADD PRIMARY KEY (id);
        -- Add unique on (series_id, provider)
        ALTER TABLE anilist_series_links
            ADD CONSTRAINT anilist_series_links_series_id_provider_key UNIQUE (series_id, provider);
    END IF;
END $$;

-- available_downloads: old unique was (library_id, series_name)
ALTER TABLE available_downloads DROP CONSTRAINT IF EXISTS available_downloads_library_id_series_name_key;
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'available_downloads_series_id_key'
    ) THEN
        ALTER TABLE available_downloads
            ADD CONSTRAINT available_downloads_series_id_key UNIQUE (series_id);
    END IF;
END $$;

-- 7. Clean up orphaned rows (series_id IS NULL after migration 0070)
DELETE FROM external_metadata_links WHERE series_id IS NULL;
DELETE FROM anilist_series_links WHERE series_id IS NULL;
DELETE FROM available_downloads WHERE series_id IS NULL;
DELETE FROM download_detection_results WHERE series_id IS NULL;

-- 8. Make series_id NOT NULL now that all data is migrated
ALTER TABLE external_metadata_links ALTER COLUMN series_id SET NOT NULL;
ALTER TABLE anilist_series_links ALTER COLUMN series_id SET NOT NULL;
ALTER TABLE available_downloads ALTER COLUMN series_id SET NOT NULL;
-- download_detection_results keeps series_id nullable (temp job data)

-- 9. Drop library_id from tables where it's now redundant (available via series.library_id JOIN)
-- Keep library_id for now on anilist_series_links and external_metadata_links for query convenience
-- Only drop from available_downloads and download_detection_results where it's truly redundant
-- Actually, keep all library_id columns for query performance (avoids extra JOIN)
