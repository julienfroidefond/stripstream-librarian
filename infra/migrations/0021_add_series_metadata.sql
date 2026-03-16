CREATE TABLE series_metadata (
    library_id UUID    NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name       TEXT    NOT NULL,
    description TEXT,
    publisher   TEXT,
    start_year  INTEGER,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (library_id, name)
);
