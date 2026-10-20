-- =============================================================================
-- Migration 0130: AI reading-list suggestion settings
-- =============================================================================
--
-- Reading-list suggestions reuse the OpenAI-compatible provider configured for
-- genre tagging (base URL, API key, model) but are gated by their own flag and
-- have their own prompt. Existing settings are preserved; only the missing keys
-- are added.

INSERT INTO app_settings (key, value)
VALUES (
    'ai_tagging',
    $json${
        "enabled": false,
        "base_url": "https://openrouter.ai/api/v1",
        "api_key": "",
        "model": "openrouter/free",
        "max_tags": 5,
        "reading_lists_enabled": false,
        "reading_list_prompt": "You are a librarian organizing a comics and manga collection. Suggest reading lists that group series sharing a strong, meaningful association (same universe, same author, same genre and audience, complementary themes, or a coherent reading order).\n\nAnalyse the existing lists: {{lists}}\n\nAnalyse these series, which are not part of any list yet: {{series}}\n\nReturn at most {{max_lists}} new lists. Only group series when the association is obvious and useful; never group unrelated series just to fill a list. A new list must contain at least 2 series.\n\nAlso propose additions of available series to existing lists when they clearly belong there. Never propose a series that is already in a list.\n\nEvery new list needs a short, specific name and a real description that explains what the series have in common (1 to 2 sentences). Write both in the same language as the existing list names when possible, otherwise in French. Add a short reason for every proposal.\n\nReturn only JSON in the form {\"additions\":[{\"list_id\":\"uuid\",\"reason\":\"...\",\"series_ids\":[\"uuid\"]}],\"new_lists\":[{\"name\":\"...\",\"description\":\"...\",\"reason\":\"...\",\"series_ids\":[\"uuid\",\"uuid\"]}]}. Use only the series ids supplied above."
    }$json$::jsonb
)
ON CONFLICT (key) DO UPDATE
SET value = $json${
    "reading_lists_enabled": false,
    "reading_list_prompt": "You are a librarian organizing a comics and manga collection. Suggest reading lists that group series sharing a strong, meaningful association (same universe, same author, same genre and audience, complementary themes, or a coherent reading order).\n\nAnalyse the existing lists: {{lists}}\n\nAnalyse these series, which are not part of any list yet: {{series}}\n\nReturn at most {{max_lists}} new lists. Only group series when the association is obvious and useful; never group unrelated series just to fill a list. A new list must contain at least 2 series.\n\nAlso propose additions of available series to existing lists when they clearly belong there. Never propose a series that is already in a list.\n\nEvery new list needs a short, specific name and a real description that explains what the series have in common (1 to 2 sentences). Write both in the same language as the existing list names when possible, otherwise in French. Add a short reason for every proposal.\n\nReturn only JSON in the form {\"additions\":[{\"list_id\":\"uuid\",\"reason\":\"...\",\"series_ids\":[\"uuid\"]}],\"new_lists\":[{\"name\":\"...\",\"description\":\"...\",\"reason\":\"...\",\"series_ids\":[\"uuid\",\"uuid\"]}]}. Use only the series ids supplied above."
}$json$::jsonb || app_settings.value;
