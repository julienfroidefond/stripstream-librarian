UPDATE app_settings
SET value = value - 'memory_max_size_mb',
    updated_at = CURRENT_TIMESTAMP
WHERE key = 'cache';
