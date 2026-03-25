ALTER TABLE libraries
  ADD COLUMN reading_status_push_mode TEXT NOT NULL DEFAULT 'manual',
  ADD COLUMN last_reading_status_push_at TIMESTAMPTZ,
  ADD COLUMN next_reading_status_push_at TIMESTAMPTZ;
