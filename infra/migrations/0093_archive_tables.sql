-- =============================================================================
-- Migration 0093: Archive tables for books and series
-- Separate tables to preserve data (especially reading progress) when
-- books/series are deleted from disk and potentially re-added later.
-- =============================================================================

-- Archived series: mirrors series without FK constraints
CREATE TABLE archived_series (
    id UUID PRIMARY KEY,
    library_id UUID,
    name TEXT NOT NULL,
    description TEXT,
    authors TEXT[] NOT NULL DEFAULT '{}',
    publishers TEXT[] NOT NULL DEFAULT '{}',
    genres TEXT[] NOT NULL DEFAULT '{}',
    start_year INTEGER,
    total_volumes INTEGER,
    status TEXT,
    locked_fields JSONB NOT NULL DEFAULT '{}',
    original_name TEXT,
    book_author TEXT,
    book_language TEXT,
    cover_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_archived_series_library_name ON archived_series(library_id, lower(name));

-- Archived books: mirrors books without FK constraints
CREATE TABLE archived_books (
    id UUID PRIMARY KEY,
    library_id UUID,
    series_id UUID,
    series_name TEXT,
    kind TEXT NOT NULL,
    format TEXT,
    title TEXT NOT NULL,
    author TEXT,
    authors TEXT[] NOT NULL DEFAULT '{}',
    volume INTEGER,
    volume_type TEXT NOT NULL DEFAULT 'regular',
    language TEXT,
    page_count INTEGER,
    thumbnail_path TEXT,
    locked_fields JSONB NOT NULL DEFAULT '{}',
    summary TEXT,
    isbn TEXT,
    publish_date TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_archived_books_library_id ON archived_books(library_id);
CREATE INDEX idx_archived_books_series_id ON archived_books(series_id);

-- Archived book files: enables restoration by abs_path
CREATE TABLE archived_book_files (
    id UUID PRIMARY KEY,
    archived_book_id UUID NOT NULL REFERENCES archived_books(id) ON DELETE CASCADE,
    format TEXT NOT NULL,
    abs_path TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    mtime TIMESTAMPTZ NOT NULL,
    fingerprint TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_archived_book_files_abs_path ON archived_book_files(abs_path);
CREATE INDEX idx_archived_book_files_book_id ON archived_book_files(archived_book_id);

-- Archived reading progress: the key data we want to preserve
CREATE TABLE archived_book_reading_progress (
    archived_book_id UUID NOT NULL REFERENCES archived_books(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'unread',
    current_page INTEGER,
    last_read_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (archived_book_id, user_id)
);
