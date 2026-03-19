INSERT INTO app_settings (key, value) VALUES
    ('qbittorrent', '{"url": "", "username": "", "password": ""}')
ON CONFLICT DO NOTHING;
