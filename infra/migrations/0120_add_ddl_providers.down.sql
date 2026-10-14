-- Revert 0120: drop the DDL-provider download tracking.
DROP TABLE IF EXISTS ddl_provider_downloads;

-- Keep any operator customisation of other settings intact; only remove the
-- seeded key when it still holds the exact default payload from the up migration.
DELETE FROM app_settings
WHERE key = 'ddl_providers'
  AND value = '{"providers": {"fourtoutici": {"enabled": false, "base_url": "https://fourtoutici.cc"}}}'::jsonb;
