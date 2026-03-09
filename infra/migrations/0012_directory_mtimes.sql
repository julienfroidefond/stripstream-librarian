CREATE TABLE directory_mtimes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    library_id UUID NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    dir_path TEXT NOT NULL,
    mtime TIMESTAMPTZ NOT NULL,
    UNIQUE(library_id, dir_path)
);
CREATE INDEX idx_directory_mtimes_library ON directory_mtimes(library_id);
