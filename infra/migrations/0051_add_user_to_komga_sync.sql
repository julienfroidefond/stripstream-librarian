ALTER TABLE komga_sync_reports ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE SET NULL;
