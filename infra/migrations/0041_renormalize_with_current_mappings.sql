-- Re-normalize series_metadata.status using current status_mappings.
-- Catches mappings added via UI after migration 0039 ran (e.g. "one shot" → "ended").

-- Exact match
UPDATE series_metadata sm
SET status = m.mapped_status, updated_at = NOW()
FROM status_mappings m
WHERE LOWER(sm.status) = m.provider_status
  AND sm.status IS NOT NULL
  AND LOWER(sm.status) != m.mapped_status;

-- Substring match
UPDATE series_metadata sm
SET status = m.mapped_status, updated_at = NOW()
FROM status_mappings m
WHERE LOWER(sm.status) LIKE '%' || m.provider_status || '%'
  AND sm.status IS NOT NULL
  AND LOWER(sm.status) != m.mapped_status;
