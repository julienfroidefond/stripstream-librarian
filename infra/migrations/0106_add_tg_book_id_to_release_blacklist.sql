ALTER TABLE release_blacklist
    ADD COLUMN tg_book_id UUID REFERENCES telegram_book_links(id) ON DELETE CASCADE;
