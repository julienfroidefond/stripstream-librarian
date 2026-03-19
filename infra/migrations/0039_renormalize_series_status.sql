-- Re-normalize series_metadata.status using the status_mappings table.
-- Batch sync was not calling normalize_series_status before, so raw provider
-- values like "Série en cours" ended up in the DB alongside "ongoing".

-- Exact match
UPDATE series_metadata sm
SET status = m.mapped_status, updated_at = NOW()
FROM status_mappings m
WHERE LOWER(sm.status) = m.provider_status
  AND sm.status IS NOT NULL
  AND LOWER(sm.status) != m.mapped_status;

-- Substring match (for values like "Série en cours" containing "en cours")
UPDATE series_metadata sm
SET status = m.mapped_status, updated_at = NOW()
FROM status_mappings m
WHERE LOWER(sm.status) LIKE '%' || m.provider_status || '%'
  AND sm.status IS NOT NULL
  AND LOWER(sm.status) != m.mapped_status;
