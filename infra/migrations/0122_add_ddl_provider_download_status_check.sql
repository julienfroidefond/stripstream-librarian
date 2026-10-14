-- Constrain the DDL download lifecycle states listed in migration 0120.
-- A new migration (rather than editing 0120) because 0120 may already be applied.
ALTER TABLE ddl_provider_downloads
  ADD CONSTRAINT ddl_provider_downloads_status_check
    CHECK (status IN ('queued', 'downloading', 'imported', 'failed', 'dismissed'));
