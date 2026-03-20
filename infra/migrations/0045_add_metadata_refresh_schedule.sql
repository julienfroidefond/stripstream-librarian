ALTER TABLE libraries
  ADD COLUMN metadata_refresh_mode TEXT NOT NULL DEFAULT 'manual',
  ADD COLUMN last_metadata_refresh_at TIMESTAMPTZ,
  ADD COLUMN next_metadata_refresh_at TIMESTAMPTZ;
