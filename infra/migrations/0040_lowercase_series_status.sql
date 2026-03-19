-- Normalize all series_metadata.status values to lowercase for consistency.
-- This fixes case mismatches like "One shot" vs "one shot".
UPDATE series_metadata
SET status = LOWER(status), updated_at = NOW()
WHERE status IS NOT NULL AND status != LOWER(status);
