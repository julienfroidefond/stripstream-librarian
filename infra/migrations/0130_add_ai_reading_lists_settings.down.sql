-- =============================================================================
-- Migration 0130 (down): remove AI reading-list suggestion settings
-- =============================================================================

UPDATE app_settings
SET value = value - 'reading_lists_enabled' - 'reading_list_prompt'
WHERE key = 'ai_tagging';
