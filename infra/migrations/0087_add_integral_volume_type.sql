-- Add 'integral' to the volume_type CHECK constraint.
ALTER TABLE books DROP CONSTRAINT IF EXISTS books_volume_type_check;
ALTER TABLE books ADD CONSTRAINT books_volume_type_check
  CHECK (volume_type IN ('regular', 'hs', 'oneshot', 'integral'));
