ALTER TABLE books ADD COLUMN IF NOT EXISTS volume_type TEXT NOT NULL DEFAULT 'regular';

ALTER TABLE books DROP CONSTRAINT IF EXISTS books_volume_type_check;
ALTER TABLE books ADD CONSTRAINT books_volume_type_check
  CHECK (volume_type IN ('regular', 'hs', 'oneshot'));
