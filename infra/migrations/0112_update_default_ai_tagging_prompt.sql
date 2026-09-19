-- Update only the original default prompt; preserve custom prompts and all other settings.
UPDATE app_settings
SET value = jsonb_set(value, '{prompt}', to_jsonb($prompt$Suggest up to {{max_tags}} relevant genres for each comic or manga series. Usually return only 1 to 3 genres; do not fill the limit when fewer genres fit.

You MUST choose tags only from this existing genre list, preserving the exact spelling and casing: {{genres}}. Never create or paraphrase a genre.

Use the supplied metadata to disambiguate the series. For a recognizable, well-known title, you may use reliable general knowledge even when its metadata is sparse. Do not make uncertain associations: omit a series if you cannot identify it with confidence. Do not select a format, medium, age category, or generic label merely because the series is a comic (for example BD or Books/Comics). Select those labels only if they are genuinely relevant genres for the series. Return only a JSON object with a suggestions array containing each series ID and its tags.

Series: {{series}}$prompt$::text))
WHERE key = 'ai_tagging'
  AND value->>'prompt' = $prompt$Suggest up to {{max_tags}} relevant genres for each comic or manga series. Usually return only 1 to 3 genres; do not fill the limit when fewer genres fit.

You MUST choose tags only from this existing genre list, preserving the exact spelling and casing: {{genres}}. Never create or paraphrase a genre.

Use the supplied metadata only, do not invent facts. Do not select a format, medium, age category, or generic label merely because the series is a comic (for example BD or Books/Comics). Select those labels only if they are genuinely relevant genres for the series. Return only a JSON object with a suggestions array containing each series ID and its tags.

Series: {{series}}$prompt$;
