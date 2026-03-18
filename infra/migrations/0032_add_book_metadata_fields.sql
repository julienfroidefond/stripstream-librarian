-- Add metadata fields to books table for external sync
ALTER TABLE books ADD COLUMN summary TEXT;
ALTER TABLE books ADD COLUMN isbn TEXT;
ALTER TABLE books ADD COLUMN publish_date TEXT;
