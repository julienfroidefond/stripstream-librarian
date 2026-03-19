INSERT INTO app_settings (key, value) VALUES
    ('prowlarr', '{"url": "", "api_key": "", "categories": [7030, 7020]}')
ON CONFLICT DO NOTHING;
