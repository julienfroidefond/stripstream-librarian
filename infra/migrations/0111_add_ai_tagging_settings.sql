INSERT INTO app_settings (key, value) VALUES
    ('ai_tagging', $${
        "enabled": false,
        "base_url": "https://openrouter.ai/api/v1",
        "api_key": "",
        "model": "openrouter/free",
        "max_tags": 5,
        "prompt": "Suggest up to {{max_tags}} relevant genres for each comic or manga series. Usually return only 1 to 3 genres; do not fill the limit when fewer genres fit.\n\nYou MUST choose tags only from this existing genre list, preserving the exact spelling and casing: {{genres}}. Never create or paraphrase a genre.\n\nUse the supplied metadata only, do not invent facts. Do not select a format, medium, age category, or generic label merely because the series is a comic (for example BD or Books/Comics). Select those labels only if they are genuinely relevant genres for the series. Return only a JSON object with a suggestions array containing each series ID and its tags.\n\nSeries: {{series}}"
    }$$::jsonb)
ON CONFLICT (key) DO NOTHING;
