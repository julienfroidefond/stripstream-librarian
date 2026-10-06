DROP INDEX IF EXISTS external_metadata_links_one_primary_per_series;
ALTER TABLE external_metadata_links DROP COLUMN IF EXISTS is_primary;
