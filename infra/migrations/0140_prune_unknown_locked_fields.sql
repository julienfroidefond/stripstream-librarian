-- Prune lock keys that no longer map to a sync-managed field.
--
-- 0139 normalized legacy lock values to booleans but kept unknown keys left by
-- older UI versions (title, series, volume, genres, ...). The API now rejects
-- them on write and only reads the keys listed in
-- `apps/api/src/metadata/field_policy.rs`; clean the rows that will never be
-- saved again. False values are dropped too: they are equivalent to an absent
-- lock.

WITH pruned AS (
    SELECT s.id,
           COALESCE(
               (
                   SELECT jsonb_object_agg(key, value)
                   FROM jsonb_each(s.locked_fields)
                   WHERE key IN (
                       'description',
                       'authors',
                       'publishers',
                       'start_year',
                       'total_volumes',
                       'status',
                       'cover_url'
                   )
                     AND value = 'true'::jsonb
               ),
               '{}'::jsonb
           ) AS locked_fields
    FROM series s
    WHERE jsonb_typeof(s.locked_fields) = 'object'
)
UPDATE series s
SET locked_fields = pruned.locked_fields
FROM pruned
WHERE s.id = pruned.id
  AND s.locked_fields IS DISTINCT FROM pruned.locked_fields;

UPDATE series
SET locked_fields = '{}'::jsonb
WHERE jsonb_typeof(locked_fields) <> 'object';

WITH pruned AS (
    SELECT b.id,
           COALESCE(
               (
                   SELECT jsonb_object_agg(key, value)
                   FROM jsonb_each(b.locked_fields)
                   WHERE key IN (
                       'summary',
                       'isbn',
                       'publish_date',
                       'language',
                       'authors'
                   )
                     AND value = 'true'::jsonb
               ),
               '{}'::jsonb
           ) AS locked_fields
    FROM books b
    WHERE jsonb_typeof(b.locked_fields) = 'object'
)
UPDATE books b
SET locked_fields = pruned.locked_fields
FROM pruned
WHERE b.id = pruned.id
  AND b.locked_fields IS DISTINCT FROM pruned.locked_fields;

UPDATE books
SET locked_fields = '{}'::jsonb
WHERE jsonb_typeof(locked_fields) <> 'object';

WITH pruned AS (
    SELECT a.id,
           COALESCE(
               (
                   SELECT jsonb_object_agg(key, value)
                   FROM jsonb_each(a.locked_fields)
                   WHERE key IN (
                       'description',
                       'authors',
                       'publishers',
                       'start_year',
                       'total_volumes',
                       'status',
                       'cover_url'
                   )
                     AND value = 'true'::jsonb
               ),
               '{}'::jsonb
           ) AS locked_fields
    FROM archived_series a
    WHERE jsonb_typeof(a.locked_fields) = 'object'
)
UPDATE archived_series a
SET locked_fields = pruned.locked_fields
FROM pruned
WHERE a.id = pruned.id
  AND a.locked_fields IS DISTINCT FROM pruned.locked_fields;

-- Restoring an archived series copies its locks back into `series`: prune
-- stale keys here too so they cannot reappear after the migration.
UPDATE archived_series
SET locked_fields = '{}'::jsonb
WHERE jsonb_typeof(locked_fields) <> 'object';
