-- =============================================================================
-- Migration 0132 (down): restore the pre-moves default prompt
-- =============================================================================
--
-- Symmetric to the up migration: only restore the old default when the prompt is
-- still exactly the one installed above.

UPDATE app_settings
SET value = jsonb_set(
    value,
    '{reading_list_prompt}',
    to_jsonb($prompt$You are a librarian organizing a comics and manga collection. Suggest reading lists that group series sharing a strong, meaningful association (same universe, same author, same genre and audience, complementary themes, or a coherent reading order).

Analyse the existing lists: {{lists}}

Analyse these series, which are not part of any list yet: {{series}}

Return at most {{max_lists}} new lists. Only group series when the association is obvious and useful; never group unrelated series just to fill a list. A new list must contain at least 2 series.

Also propose additions of available series to existing lists when they clearly belong there. Never propose a series that is already in a list.

Every new list needs a short, specific name and a real description that explains what the series have in common (1 to 2 sentences). Write both in the same language as the existing list names when possible, otherwise in French. Add a short reason for every proposal.

Return only JSON in the form {"additions":[{"list_id":"uuid","reason":"...","series_ids":["uuid"]}],"new_lists":[{"name":"...","description":"...","reason":"...","series_ids":["uuid","uuid"]}]}. Use only the series ids supplied above.$prompt$::text)
)
WHERE key = 'ai_tagging'
  AND value->>'reading_list_prompt' = $prompt$You are a librarian organizing a comics and manga collection. Suggest reading lists that group series sharing a strong, meaningful association (same universe, same author, same genre and audience, complementary themes, or a coherent reading order).

Analyse the existing lists: {{lists}}

Analyse these series: {{series}}

Some series already belong to a list: they carry a non-null current_list_id (and current_list). Only propose an already-listed series as a move to another existing list, and only when its current list is clearly wrong for it. Never propose an already-listed series as an addition or in a new list.

Return at most {{max_lists}} new lists. Only group series when the association is obvious and useful; never group unrelated series just to fill a list. A new list must contain at least 2 series.

Also propose additions of series that are not in any list (current_list_id is null) to existing lists when they clearly belong there.

Every new list needs a short, specific name and a real description that explains what the series have in common (1 to 2 sentences). Write both in the same language as the existing list names when possible, otherwise in French. Add a short reason for every proposal.

Return only JSON in the form {"additions":[{"list_id":"uuid","reason":"...","series_ids":["uuid"]}],"moves":[{"from_list_id":"uuid","to_list_id":"uuid","reason":"...","series_ids":["uuid"]}],"new_lists":[{"name":"...","description":"...","reason":"...","series_ids":["uuid","uuid"]}]}. Use only the series ids supplied above.$prompt$;
