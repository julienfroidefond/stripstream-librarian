-- Corrective migration: add series_metadata columns that may be missing if migrations
-- 0022-0033 were baselined (marked applied) without actually running their SQL.
-- All statements use IF NOT EXISTS / idempotent patterns so re-running is safe.

-- From 0022: add authors + publishers arrays, remove old singular publisher column
ALTER TABLE series_metadata ADD COLUMN IF NOT EXISTS authors    TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE series_metadata ADD COLUMN IF NOT EXISTS publishers TEXT[] NOT NULL DEFAULT '{}';

-- Migrate old singular publisher value if the column still exists
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'series_metadata' AND column_name = 'publisher'
    ) THEN
        UPDATE series_metadata
           SET publishers = ARRAY[publisher]
         WHERE publisher IS NOT NULL AND publisher != '' AND cardinality(publishers) = 0;
        ALTER TABLE series_metadata DROP COLUMN publisher;
    END IF;
END $$;

-- From 0030: locked_fields
ALTER TABLE series_metadata ADD COLUMN IF NOT EXISTS locked_fields JSONB NOT NULL DEFAULT '{}';

-- From 0031: total_volumes
ALTER TABLE series_metadata ADD COLUMN IF NOT EXISTS total_volumes INTEGER;

-- From 0033: status
ALTER TABLE series_metadata ADD COLUMN IF NOT EXISTS status TEXT;
