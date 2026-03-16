CREATE TABLE IF NOT EXISTS komga_sync_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    komga_url TEXT NOT NULL,
    total_komga_read BIGINT NOT NULL DEFAULT 0,
    matched BIGINT NOT NULL DEFAULT 0,
    already_read BIGINT NOT NULL DEFAULT 0,
    newly_marked BIGINT NOT NULL DEFAULT 0,
    unmatched JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
