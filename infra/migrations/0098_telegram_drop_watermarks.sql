-- Remove iteration watermarks — sync is now search-based, these columns are unused
ALTER TABLE telegram_sources
  DROP COLUMN IF EXISTS last_message_id,
  DROP COLUMN IF EXISTS oldest_message_id;
