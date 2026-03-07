CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO app_settings (key, value) VALUES 
    ('image_processing', '{"format": "webp", "quality": 85, "filter": "lanczos3", "max_width": 2160}'),
    ('cache', '{"enabled": true, "directory": "/tmp/stripstream-image-cache", "max_size_mb": 10000}'),
    ('limits', '{"concurrent_renders": 4, "timeout_seconds": 12, "rate_limit_per_second": 120}')
ON CONFLICT DO NOTHING;
