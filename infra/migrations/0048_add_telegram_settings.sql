INSERT INTO app_settings (key, value) VALUES
    ('telegram', '{"bot_token": "", "chat_id": "", "enabled": false, "events": {"job_completed": true, "job_failed": true, "job_cancelled": true, "metadata_approved": true}}')
ON CONFLICT DO NOTHING;
