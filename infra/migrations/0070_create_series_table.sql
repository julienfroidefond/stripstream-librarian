-- =============================================================================
-- Migration 0070: Create proper `series` table with UUID PK
-- Fuses series_metadata into series. All related tables get series_id FK.
-- =============================================================================

-- 1. Create the series table (fusion of series_metadata)
CREATE TABLE IF NOT EXISTS series (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id UUID NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    authors TEXT[] NOT NULL DEFAULT '{}',
    publishers TEXT[] NOT NULL DEFAULT '{}',
    start_year INTEGER,
    total_volumes INTEGER,
    status TEXT,
    locked_fields JSONB NOT NULL DEFAULT '{}',
    original_name TEXT,
    book_author TEXT,
    book_language TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (library_id, name)
);

CREATE INDEX IF NOT EXISTS idx_series_library_id ON series(library_id);

-- 2. Populate from existing data

-- From books (all series with at least one book)
INSERT INTO series (library_id, name)
SELECT DISTINCT b.library_id, COALESCE(NULLIF(b.series, ''), 'unclassified')
FROM books b
ON CONFLICT (library_id, name) DO NOTHING;

-- From series_metadata (series with metadata but possibly no books)
INSERT INTO series (library_id, name)
SELECT sm.library_id, sm.name FROM series_metadata sm
ON CONFLICT (library_id, name) DO NOTHING;

-- Merge series_metadata columns into series
UPDATE series s SET
    description = sm.description,
    authors = sm.authors,
    publishers = sm.publishers,
    start_year = sm.start_year,
    total_volumes = sm.total_volumes,
    status = sm.status,
    locked_fields = sm.locked_fields,
    original_name = sm.original_name,
    created_at = sm.created_at,
    updated_at = sm.updated_at
FROM series_metadata sm
WHERE sm.library_id = s.library_id AND sm.name = s.name;

-- 3. Add series_id FK to books
ALTER TABLE books ADD COLUMN IF NOT EXISTS series_id UUID REFERENCES series(id) ON DELETE SET NULL;
UPDATE books b SET series_id = s.id
FROM series s
WHERE s.library_id = b.library_id
  AND s.name = COALESCE(NULLIF(b.series, ''), 'unclassified')
  AND b.series_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_books_series_id ON books(series_id);

-- 4. Add series_id FK to external_metadata_links
ALTER TABLE external_metadata_links ADD COLUMN IF NOT EXISTS series_id UUID REFERENCES series(id) ON DELETE CASCADE;
UPDATE external_metadata_links eml SET series_id = s.id
FROM series s
WHERE s.library_id = eml.library_id AND LOWER(s.name) = LOWER(eml.series_name)
  AND eml.series_id IS NULL;

-- 5. Add series_id FK to anilist_series_links
ALTER TABLE anilist_series_links ADD COLUMN IF NOT EXISTS series_id UUID REFERENCES series(id) ON DELETE CASCADE;
UPDATE anilist_series_links asl SET series_id = s.id
FROM series s
WHERE s.library_id = asl.library_id AND LOWER(s.name) = LOWER(asl.series_name)
  AND asl.series_id IS NULL;

-- 6. Add series_id FK to available_downloads
ALTER TABLE available_downloads ADD COLUMN IF NOT EXISTS series_id UUID REFERENCES series(id) ON DELETE CASCADE;
UPDATE available_downloads ad SET series_id = s.id
FROM series s
WHERE s.library_id = ad.library_id AND LOWER(s.name) = LOWER(ad.series_name)
  AND ad.series_id IS NULL;

-- 7. Add series_id FK to download_detection_results
ALTER TABLE download_detection_results ADD COLUMN IF NOT EXISTS series_id UUID REFERENCES series(id) ON DELETE CASCADE;
UPDATE download_detection_results ddr SET series_id = s.id
FROM series s
WHERE s.library_id = ddr.library_id AND LOWER(s.name) = LOWER(ddr.series_name)
  AND ddr.series_id IS NULL;

-- NOTE: Old TEXT columns (books.series, *.series_name) are kept for now.
-- They will be dropped in a future migration (0071) once all code is migrated.
