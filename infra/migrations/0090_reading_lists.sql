-- Reading lists: named, ordered collections of series spanning multiple libraries
CREATE TABLE reading_lists (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE reading_list_items (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id    UUID NOT NULL REFERENCES reading_lists(id) ON DELETE CASCADE,
    series_id  UUID NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    position   INT  NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (list_id, series_id)
);

CREATE INDEX reading_list_items_list_position ON reading_list_items (list_id, position);
