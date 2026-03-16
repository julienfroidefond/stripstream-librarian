ALTER TABLE komga_sync_reports ADD COLUMN IF NOT EXISTS newly_marked_books JSONB NOT NULL DEFAULT '[]';
