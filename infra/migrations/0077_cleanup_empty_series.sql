-- Cleanup: delete series that have no books AND no approved metadata link.
-- Series added via discovery (with an approved metadata link) are preserved.
-- This is a one-time cleanup before empty series become visible in the UI.
DELETE FROM series
WHERE id IN (
    SELECT s.id
    FROM series s
    LEFT JOIN books b ON b.series_id = s.id
    LEFT JOIN external_metadata_links eml ON eml.series_id = s.id AND eml.status = 'approved'
    WHERE b.id IS NULL
      AND eml.id IS NULL
);
