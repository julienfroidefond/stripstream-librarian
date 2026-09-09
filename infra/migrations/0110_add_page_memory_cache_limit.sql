UPDATE app_settings
SET value = value || '{"memory_max_size_mb": 128}'::jsonb,
    updated_at = CURRENT_TIMESTAMP
WHERE key = 'cache'
  AND NOT (value ? 'memory_max_size_mb');
