ALTER TABLE libraries ADD COLUMN download_detection_mode VARCHAR NOT NULL DEFAULT 'manual';
ALTER TABLE libraries ADD COLUMN next_download_detection_at TIMESTAMPTZ;
ALTER TABLE libraries ADD COLUMN last_download_detection_at TIMESTAMPTZ;
