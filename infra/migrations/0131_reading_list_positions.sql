-- =============================================================================
-- Migration 0131: make reading-list item positions dense and unique
-- =============================================================================
--
-- Historical code read MAX(position) on insert and deleted rows without
-- renumbering, so lists accumulated gaps and could even get duplicate
-- positions under concurrency. Normalise every list to 0..n-1 first, then
-- enforce the invariant in the database.
--
-- The constraint is DEFERRABLE INITIALLY DEFERRED because renumbering and
-- reordering legitimately swap positions within a transaction (e.g. position 0
-- becomes 1 while position 1 becomes 0); the check only has to hold at COMMIT.

-- 1. Renumber every list densely. Ties on the legacy `position` are broken by
--    creation order then id, which keeps the existing visual order stable.
WITH ranked AS (
    SELECT
        id,
        (ROW_NUMBER() OVER (
            PARTITION BY list_id
            ORDER BY position, created_at, id
        ) - 1)::int AS new_position
    FROM reading_list_items
)
UPDATE reading_list_items AS rli
SET position = ranked.new_position
FROM ranked
WHERE rli.id = ranked.id
  AND rli.position IS DISTINCT FROM ranked.new_position;

-- 2. Replace the non-unique lookup index with the unique constraint (which
--    creates its own index on (list_id, position)).
DROP INDEX IF EXISTS reading_list_items_list_position;

ALTER TABLE reading_list_items
    DROP CONSTRAINT IF EXISTS reading_list_items_list_id_position_key;

ALTER TABLE reading_list_items
    ADD CONSTRAINT reading_list_items_list_id_position_key
    UNIQUE (list_id, position) DEFERRABLE INITIALLY DEFERRED;
