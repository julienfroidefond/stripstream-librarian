ALTER TABLE komga_sync_reports ADD COLUMN IF NOT EXISTS matched_books JSONB NOT NULL DEFAULT '[]';
