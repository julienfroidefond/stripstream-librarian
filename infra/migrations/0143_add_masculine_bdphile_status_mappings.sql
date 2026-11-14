-- BDphile reports series statuses in the masculine form ("Terminé", "Annulé",
-- "Abandonné", "Suspendu"). The feminine mappings seeded by 0038 do not match
-- them (neither exactly nor as a substring), so they were stored raw.
INSERT INTO status_mappings (provider_status, mapped_status) VALUES
    ('terminé', 'ended'),
    ('annulé', 'cancelled'),
    ('abandonné', 'cancelled'),
    ('abandonnée', 'cancelled'),
    ('suspendu', 'hiatus')
ON CONFLICT (provider_status) DO NOTHING;

-- Re-normalize series already synced with the raw masculine status.
UPDATE series s
SET status = m.mapped_status, updated_at = NOW()
FROM status_mappings m
WHERE LOWER(s.status) = m.provider_status
  AND m.provider_status IN ('terminé', 'annulé', 'abandonné', 'abandonnée', 'suspendu')
  AND m.mapped_status IS NOT NULL
  AND s.status IS DISTINCT FROM m.mapped_status;
