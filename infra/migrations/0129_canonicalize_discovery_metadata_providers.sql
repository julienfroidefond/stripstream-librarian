-- =============================================================================
-- Migration 0129: canonicalize discovery source ids in metadata links (#112)
-- =============================================================================
--
-- Discovery exposes several SensCritique-backed ranking lists (`senscritique_bd`,
-- `sc_trending_bd`, `sc_trending_manga`, `sc_best_bd`, `sc_best_manga`). They are
-- discovery *sources*, not metadata providers: they all resolve to the
-- `senscritique` provider.
--
-- The add-to-library path only normalized the `sc_` prefix, so links created
-- from the "SensCritique / Top BD" tab were stored with `provider =
-- 'senscritique_bd'`. Metadata refresh then failed with
-- "Unknown provider: senscritique_bd" and the link never synced again.
--
-- Merge every alias onto `senscritique` while keeping a single link per series
-- (the UNIQUE (series_id, provider) constraint forbids duplicates).

DO $$
DECLARE
    discovery_aliases text[] := ARRAY[
        'senscritique_bd',
        'sc_trending_bd',
        'sc_trending_manga',
        'sc_best_bd',
        'sc_best_manga'
    ];
BEGIN
    -- Keep one link per series among {senscritique} ∪ aliases: prefer an approved
    -- link, then the primary one, then the oldest. The others are dropped
    -- (their external_book_metadata rows cascade).
    WITH ranked AS (
        SELECT l.id,
               ROW_NUMBER() OVER (
                   PARTITION BY l.series_id
                   ORDER BY (l.status = 'approved') DESC,
                            l.is_primary DESC,
                            l.approved_at ASC NULLS LAST,
                            l.created_at ASC,
                            l.id ASC
               ) AS rank
        FROM external_metadata_links l
        WHERE l.provider = 'senscritique'
           OR l.provider = ANY(discovery_aliases)
    )
    DELETE FROM external_metadata_links
    WHERE id IN (SELECT id FROM ranked WHERE rank > 1);

    UPDATE external_metadata_links
    SET provider = 'senscritique',
        updated_at = NOW()
    WHERE provider = ANY(discovery_aliases);
END $$;
