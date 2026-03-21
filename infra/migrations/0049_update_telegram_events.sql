-- Update telegram events from 4 generic toggles to 12 granular toggles
UPDATE app_settings
SET value = jsonb_set(
    value,
    '{events}',
    '{"scan_completed": true, "scan_failed": true, "scan_cancelled": true, "thumbnail_completed": true, "thumbnail_failed": true, "conversion_completed": true, "conversion_failed": true, "metadata_approved": true, "metadata_batch_completed": true, "metadata_batch_failed": true, "metadata_refresh_completed": true, "metadata_refresh_failed": true}'::jsonb
)
WHERE key = 'telegram';
