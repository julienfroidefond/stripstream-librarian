CREATE TABLE IF NOT EXISTS sync_metadata (
    id INTEGER PRIMARY KEY,
    last_meili_sync TIMESTAMPTZ
);

INSERT INTO sync_metadata (id, last_meili_sync) VALUES (1, NULL) ON CONFLICT DO NOTHING;
