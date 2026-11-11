-- Normalize legacy locked_fields values to strict JSON booleans.
--
-- The API and the metadata sync SQL now treat locks as booleans only. Older
-- rows may contain strings ("true") or numbers (1) left by previous tolerant
-- readers; the generated SQL casts values with `::boolean`, which would either
-- be ignored by the strict Rust helper or fail on unrecognized strings.
-- Rebuild each lock object with booleans and drop values that cannot be
-- represented unambiguously.

WITH normalized AS (
    SELECT s.id,
           COALESCE(
               (
                   SELECT jsonb_object_agg(
                       key,
                       CASE jsonb_typeof(value)
                           WHEN 'boolean' THEN value
                           WHEN 'string' THEN to_jsonb(lower(value #>> '{}') = 'true')
                           WHEN 'number' THEN to_jsonb((value #>> '{}')::numeric <> 0)
                       END
                   )
                   FROM jsonb_each(s.locked_fields)
                   WHERE jsonb_typeof(value) = 'boolean'
                      OR (
                          jsonb_typeof(value) = 'string'
                          AND lower(value #>> '{}') IN ('true', 'false')
                      )
                      OR jsonb_typeof(value) = 'number'
               ),
               '{}'::jsonb
           ) AS locked_fields
    FROM series s
    WHERE jsonb_typeof(s.locked_fields) = 'object'
)
UPDATE series s
SET locked_fields = normalized.locked_fields
FROM normalized
WHERE s.id = normalized.id
  AND s.locked_fields IS DISTINCT FROM normalized.locked_fields;

UPDATE series
SET locked_fields = '{}'::jsonb
WHERE jsonb_typeof(locked_fields) <> 'object';

WITH normalized AS (
    SELECT b.id,
           COALESCE(
               (
                   SELECT jsonb_object_agg(
                       key,
                       CASE jsonb_typeof(value)
                           WHEN 'boolean' THEN value
                           WHEN 'string' THEN to_jsonb(lower(value #>> '{}') = 'true')
                           WHEN 'number' THEN to_jsonb((value #>> '{}')::numeric <> 0)
                       END
                   )
                   FROM jsonb_each(b.locked_fields)
                   WHERE jsonb_typeof(value) = 'boolean'
                      OR (
                          jsonb_typeof(value) = 'string'
                          AND lower(value #>> '{}') IN ('true', 'false')
                      )
                      OR jsonb_typeof(value) = 'number'
               ),
               '{}'::jsonb
           ) AS locked_fields
    FROM books b
    WHERE jsonb_typeof(b.locked_fields) = 'object'
)
UPDATE books b
SET locked_fields = normalized.locked_fields
FROM normalized
WHERE b.id = normalized.id
  AND b.locked_fields IS DISTINCT FROM normalized.locked_fields;

UPDATE books
SET locked_fields = '{}'::jsonb
WHERE jsonb_typeof(locked_fields) <> 'object';
