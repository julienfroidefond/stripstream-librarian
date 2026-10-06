-- Designate a primary metadata provider per series.
--
-- `external_metadata_links` already allows several providers per series
-- (UNIQUE (series_id, provider) since migration 0071); this migration adds the
-- notion of a *primary* link. Metadata sync uses the primary first and falls
-- back to the secondary links for missing fields.
--
-- At most one approved link per series may be primary, enforced by a partial
-- unique index below.

ALTER TABLE external_metadata_links
    ADD COLUMN IF NOT EXISTS is_primary BOOLEAN NOT NULL DEFAULT false;

-- Backfill: promote the oldest approved link of each series to primary.
-- (Before this change the API only ever kept a single approved link per series,
-- so this is a no-op semantically, but stays correct if several exist.)
UPDATE external_metadata_links eml
SET is_primary = true
WHERE eml.status = 'approved'
  AND eml.id = (
      SELECT e2.id
      FROM external_metadata_links e2
      WHERE e2.series_id = eml.series_id
        AND e2.status = 'approved'
      ORDER BY e2.approved_at ASC NULLS LAST, e2.id ASC
      LIMIT 1
  );

CREATE UNIQUE INDEX IF NOT EXISTS external_metadata_links_one_primary_per_series
    ON external_metadata_links (series_id)
    WHERE is_primary AND status = 'approved';
