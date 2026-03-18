-- Add locked_fields to series_metadata and books so that manually edited
-- fields can be protected from external-metadata sync overwrites.
-- The JSON object maps field names to true, e.g. {"authors": true, "description": true}.

ALTER TABLE series_metadata ADD COLUMN locked_fields JSONB NOT NULL DEFAULT '{}';
ALTER TABLE books ADD COLUMN locked_fields JSONB NOT NULL DEFAULT '{}';
