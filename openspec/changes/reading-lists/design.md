# Reading Lists — Design

## Database

```sql
-- Migration: XXXX_reading_lists.sql
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

CREATE INDEX ON reading_list_items (list_id, position);
```

Position is a plain integer. Reordering sends a full ordered array of series_ids → backend reassigns positions 0,1,2,…

## API — Admin scope

| Method | Path | Description |
|--------|------|-------------|
| POST   | /reading-lists | Create list |
| GET    | /reading-lists | List all |
| GET    | /reading-lists/:id | Get one with items |
| PATCH  | /reading-lists/:id | Update name/description |
| DELETE | /reading-lists/:id | Delete list |
| POST   | /reading-lists/:id/series | Add series |
| DELETE | /reading-lists/:id/series/:series_id | Remove series |
| PUT    | /reading-lists/:id/series/reorder | Full reorder (body: [series_id, …]) |

## API — Client scope (read)

| Method | Path | Description |
|--------|------|-------------|
| GET    | /reading-lists | All lists (name, id, item count) |
| GET    | /reading-lists/:id | List + series (id, name, cover_url, metadata provider) |

## Series DTO in client response

```json
{
  "id": "uuid",
  "name": "Astérix",
  "cover_url": "https://…",
  "provider": "senscritique",
  "external_id": "franchise:123",
  "external_url": "https://www.senscritique.com/…",
  "library_id": "uuid",
  "library_name": "BD"
}
```

## Backoffice UI

- New `/reading-lists` page: table of lists
- Detail page `/reading-lists/:id`: drag-to-reorder (or up/down buttons), search & add series, remove
- Nav entry in sidebar
