-- =============================================================================
-- Migration 0131 (down): allow duplicate reading-list positions again
-- =============================================================================

ALTER TABLE reading_list_items
    DROP CONSTRAINT IF EXISTS reading_list_items_list_id_position_key;

CREATE INDEX IF NOT EXISTS reading_list_items_list_position
    ON reading_list_items (list_id, position);
