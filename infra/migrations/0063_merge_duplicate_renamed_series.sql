-- Fix duplicate series created when the scanner recreated a filesystem-named
-- series alongside a user-renamed one.
-- Example: user renamed "LES MYTHICS" → "Mythics" via UI, but the scanner
-- also kept (or recreated) a "LES MYTHICS" entry. This migration merges
-- all such duplicates by using the original_name column added in 0062.
--
-- For each canonical series (original_name IS NOT NULL), if a duplicate still
-- exists with name = original_name in the same library, we:
--   1. Reassign books from the duplicate to the canonical series
--   2. Update external_metadata_links
--   3. Update anilist_series_links
--   4. Delete the duplicate series_metadata row

-- 1. Reassign books
UPDATE books b
SET series = sm_canonical.name
FROM series_metadata sm_canonical
JOIN series_metadata sm_dup
    ON sm_dup.library_id = sm_canonical.library_id
   AND sm_dup.name = sm_canonical.original_name
WHERE b.library_id = sm_canonical.library_id
  AND b.series = sm_dup.name
  AND sm_canonical.original_name IS NOT NULL;

-- 2. Update external_metadata_links (skip if a row for the canonical name already exists)
UPDATE external_metadata_links eml
SET series_name = sm_canonical.name
FROM series_metadata sm_canonical
JOIN series_metadata sm_dup
    ON sm_dup.library_id = sm_canonical.library_id
   AND sm_dup.name = sm_canonical.original_name
WHERE eml.library_id = sm_canonical.library_id
  AND eml.series_name = sm_dup.name
  AND sm_canonical.original_name IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM external_metadata_links eml2
      WHERE eml2.library_id = eml.library_id
        AND eml2.series_name = sm_canonical.name
        AND eml2.provider = eml.provider
  );

-- 3. Update anilist_series_links (skip if a row for the canonical name already exists)
UPDATE anilist_series_links asl
SET series_name = sm_canonical.name
FROM series_metadata sm_canonical
JOIN series_metadata sm_dup
    ON sm_dup.library_id = sm_canonical.library_id
   AND sm_dup.name = sm_canonical.original_name
WHERE asl.library_id = sm_canonical.library_id
  AND asl.series_name = sm_dup.name
  AND sm_canonical.original_name IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM anilist_series_links asl2
      WHERE asl2.library_id = asl.library_id
        AND asl2.series_name = sm_canonical.name
        AND asl2.provider = asl.provider
  );

-- 4. Delete duplicate series_metadata rows
DELETE FROM series_metadata sm_dup
USING series_metadata sm_canonical
WHERE sm_canonical.library_id = sm_dup.library_id
  AND sm_canonical.original_name = sm_dup.name
  AND sm_canonical.original_name IS NOT NULL;
