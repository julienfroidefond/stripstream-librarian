-- Track oldest processed message per source for incremental historical backfill
ALTER TABLE telegram_sources
  ADD COLUMN oldest_message_id BIGINT NOT NULL DEFAULT 0;
