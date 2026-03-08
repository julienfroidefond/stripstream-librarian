ALTER TABLE books ADD COLUMN IF NOT EXISTS thumbnail_path TEXT;

INSERT INTO app_settings (key, value) VALUES 
    ('thumbnail', '{"enabled": true, "width": 300, "height": 400, "quality": 80, "format": "webp", "directory": "/data/thumbnails"}')
ON CONFLICT (key) DO UPDATE SET value = '{"enabled": true, "width": 300, "height": 400, "quality": 80, "format": "webp", "directory": "/data/thumbnails"}'::jsonb;
