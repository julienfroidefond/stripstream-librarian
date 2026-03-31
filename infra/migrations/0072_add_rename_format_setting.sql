INSERT INTO app_settings (key, value)
VALUES ('rename_format', '"{series_name} - T{volume_padded} - {title}"')
ON CONFLICT (key) DO NOTHING;
