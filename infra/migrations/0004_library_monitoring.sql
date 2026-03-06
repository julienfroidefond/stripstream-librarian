-- Migration: Surveillance automatique des libraries (T23)
-- Ajout des colonnes pour la configuration du scan automatique

ALTER TABLE libraries 
  ADD COLUMN IF NOT EXISTS monitor_enabled BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN IF NOT EXISTS scan_mode TEXT NOT NULL DEFAULT 'manual' CHECK (scan_mode IN ('manual', 'hourly', 'daily', 'weekly')),
  ADD COLUMN IF NOT EXISTS last_scan_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS next_scan_at TIMESTAMPTZ;

-- Index pour trouver rapidement les libraries à scanner
CREATE INDEX IF NOT EXISTS idx_libraries_monitor_enabled ON libraries(monitor_enabled);
CREATE INDEX IF NOT EXISTS idx_libraries_next_scan_at ON libraries(next_scan_at) WHERE monitor_enabled = TRUE;

COMMENT ON COLUMN libraries.monitor_enabled IS 'Active la surveillance automatique de la library';
COMMENT ON COLUMN libraries.scan_mode IS 'Mode de scan: manual, hourly (60min), daily (1440min), weekly (10080min)';
COMMENT ON COLUMN libraries.last_scan_at IS 'Date du dernier scan (manuel ou automatique)';
COMMENT ON COLUMN libraries.next_scan_at IS 'Date du prochain scan automatique prévu';
