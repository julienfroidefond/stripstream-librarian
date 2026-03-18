CREATE TABLE external_metadata_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id UUID NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    series_name TEXT NOT NULL,
    provider TEXT NOT NULL,
    external_id TEXT NOT NULL,
    external_url TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    confidence REAL,
    metadata_json JSONB NOT NULL DEFAULT '{}',
    total_volumes_external INTEGER,
    matched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    approved_at TIMESTAMPTZ,
    synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (library_id, series_name, provider)
);

CREATE TABLE external_book_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    link_id UUID NOT NULL REFERENCES external_metadata_links(id) ON DELETE CASCADE,
    book_id UUID REFERENCES books(id) ON DELETE SET NULL,
    external_book_id TEXT,
    volume_number INTEGER,
    title TEXT,
    authors TEXT[] NOT NULL DEFAULT '{}',
    isbn TEXT,
    summary TEXT,
    cover_url TEXT,
    page_count INTEGER,
    language TEXT,
    publish_date TEXT,
    metadata_json JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_eml_library_series ON external_metadata_links(library_id, series_name);
CREATE INDEX idx_eml_status ON external_metadata_links(status);
CREATE INDEX idx_ebm_link_id ON external_book_metadata(link_id);
CREATE INDEX idx_ebm_book_id ON external_book_metadata(book_id);
