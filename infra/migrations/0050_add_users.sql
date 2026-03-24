CREATE TABLE users (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username   TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Les tokens read ont un user_id obligatoire, les tokens admin NULL
ALTER TABLE api_tokens ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;

-- Rendre book_reading_progress par user
ALTER TABLE book_reading_progress DROP CONSTRAINT book_reading_progress_pkey;
ALTER TABLE book_reading_progress ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;

-- Créer un user par défaut si des données existantes doivent être migrées
INSERT INTO users (id, username)
    SELECT '00000000-0000-0000-0000-000000000001', 'default'
    WHERE EXISTS (
        SELECT 1 FROM book_reading_progress WHERE user_id IS NULL
        UNION ALL
        SELECT 1 FROM api_tokens WHERE scope = 'read' AND user_id IS NULL
    );

-- Rattacher les anciennes progressions de lecture au user default
UPDATE book_reading_progress
    SET user_id = '00000000-0000-0000-0000-000000000001'
    WHERE user_id IS NULL;

-- Rattacher les anciens tokens read au user default
UPDATE api_tokens
    SET user_id = '00000000-0000-0000-0000-000000000001'
    WHERE scope = 'read' AND user_id IS NULL;

ALTER TABLE book_reading_progress ALTER COLUMN user_id SET NOT NULL;
ALTER TABLE book_reading_progress ADD PRIMARY KEY (book_id, user_id);
DROP INDEX IF EXISTS idx_book_reading_progress_status;
CREATE INDEX idx_book_reading_progress_status ON book_reading_progress(status, user_id);
