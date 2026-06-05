-- =============================================================================
-- Migration 0094: Telegram Monitor — channels + book links
-- =============================================================================

-- Telegram channels to monitor for book files
CREATE TABLE telegram_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    channel_username TEXT NOT NULL,
    channel_title TEXT,
    library_id UUID REFERENCES libraries(id) ON DELETE SET NULL,
    last_message_id BIGINT NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(channel_username)
);

-- Book files discovered in Telegram channels
CREATE TABLE telegram_book_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES telegram_sources(id) ON DELETE CASCADE,
    message_id BIGINT NOT NULL,
    filename TEXT NOT NULL,
    file_size BIGINT,
    mime_type TEXT,
    message_text TEXT,
    status TEXT NOT NULL DEFAULT 'available',
    -- available | downloading | imported | failed | dismissed
    library_id UUID REFERENCES libraries(id) ON DELETE SET NULL,
    book_id UUID REFERENCES books(id) ON DELETE SET NULL,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(source_id, message_id)
);

CREATE INDEX idx_telegram_book_links_source ON telegram_book_links(source_id);
CREATE INDEX idx_telegram_book_links_status ON telegram_book_links(status);
