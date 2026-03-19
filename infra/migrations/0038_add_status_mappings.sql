-- Status mappings: many provider statuses → one target status (existing in series_metadata.status)
CREATE TABLE status_mappings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_status TEXT NOT NULL UNIQUE,
    mapped_status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Pre-populate with current hardcoded mappings from normalize_series_status
INSERT INTO status_mappings (provider_status, mapped_status) VALUES
    -- AniList
    ('finished', 'ended'),
    ('releasing', 'ongoing'),
    ('not_yet_released', 'upcoming'),
    ('cancelled', 'cancelled'),
    ('hiatus', 'hiatus'),
    -- Bédéthèque (French)
    ('finie', 'ended'),
    ('terminée', 'ended'),
    ('en cours', 'ongoing'),
    ('suspendue', 'hiatus'),
    ('annulée', 'cancelled'),
    ('arrêtée', 'cancelled');
