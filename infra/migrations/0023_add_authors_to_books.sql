ALTER TABLE books
    ADD COLUMN authors TEXT[] NOT NULL DEFAULT '{}';

-- Migrate existing scalar author → authors array
UPDATE books SET authors = ARRAY[author] WHERE author IS NOT NULL AND author != '';
