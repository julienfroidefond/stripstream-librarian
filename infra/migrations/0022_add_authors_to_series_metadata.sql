ALTER TABLE series_metadata
    ADD COLUMN authors    TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN publishers TEXT[] NOT NULL DEFAULT '{}';

-- Migrate existing scalar publisher → publishers array
UPDATE series_metadata SET publishers = ARRAY[publisher] WHERE publisher IS NOT NULL AND publisher != '';

ALTER TABLE series_metadata DROP COLUMN publisher;
