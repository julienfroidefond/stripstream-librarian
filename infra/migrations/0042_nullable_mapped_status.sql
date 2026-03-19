-- Allow mapped_status to be NULL to represent "known but unmapped" provider statuses.
-- Clicking X in the UI will set mapped_status to NULL instead of deleting the row.
ALTER TABLE status_mappings ALTER COLUMN mapped_status DROP NOT NULL;
