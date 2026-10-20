---
name: Stripstream Genres
description: Audit, propose and apply genres on Stripstream series (production or local API) — find untagged series, map them to the existing genre taxonomy, bulk-assign via the API, verify results, and report orphan/duplicate series. Use when the user says series lack genres, asks to tag/retag genres, clean up genre taxonomy, or run genre operations against production.
---

# Stripstream Genres

Operate on series genres through the Stripstream REST API, with an explicit
audit → propose → validate → apply → verify loop. Production writes are
irreversible (no audit log, no undo), so never apply anything without an
explicit user validation of the mapping.

## 1. Environment and credentials

| Target | API base | Credentials |
| ------ | -------- | ----------- |
| Production | `https://stripstream-api.julienfroidefond.com` | `scripts/.env` |
| Local | `http://localhost:7080` | `scripts/.env` or your local admin token |

`scripts/.env` holds `STRIPSTREAM_API_URL` and `STRIPSTREAM_API_TOKEN`
(admin scope, Bearer auth). It is git-ignored — never print the token, never
commit it, never echo the header.

```bash
set -a; source scripts/.env; set +a
curl -s -m 20 -H "Authorization: Bearer $STRIPSTREAM_API_TOKEN" "$STRIPSTREAM_API_URL/health"
```

If `/health` does not return 200, stop and report instead of guessing an
endpoint.

## 2. Endpoints used by this skill

| Method | Path | Purpose |
| ------ | ---- | ------- |
| `GET` | `/genres` | All genres with `series_count` (the taxonomy) |
| `GET` | `/genres/untagged-series` | Series with an empty `genres` array (max 500) |
| `GET` | `/series?genre=<name>&limit=N` | Series carrying one genre (reference conventions) |
| `GET` | `/series?q=<name>` | Accent/case-insensitive series search |
| `GET` | `/series/{id}/metadata` | `SeriesMetadata`: `series_name`, `genres`, `authors`, `publishers`, `description`, `start_year`, `total_volumes` |
| `POST` | `/genres/assign` | Bulk assign: `{"genre": "<name>", "series_ids": ["<uuid>", ...]}` |
| `PATCH` | `/genres/{name}` | Rename a genre across all series |
| `DELETE` | `/genres/{name}` | Remove a genre from all series |
| `PATCH` | `/series/{id}` | Replace the `genres` array of one series |
| `POST` | `/genres/ai-suggest` | AI proposals for 1–25 series (read-only, requires IAG configured) |

Note: `GET /series/{id}/metadata` returns `series_name`, not `name`.
`GET /admin/series` returns 404 — do not use it.

## 3. Audit

```bash
set -a; source scripts/.env; set +a

# Existing taxonomy (names include emoji prefixes — copy them verbatim)
curl -s -H "Authorization: Bearer $STRIPSTREAM_API_TOKEN" "$STRIPSTREAM_API_URL/genres"

# Untagged series, with description/authors to build the mapping
curl -s -H "Authorization: Bearer $STRIPSTREAM_API_TOKEN" "$STRIPSTREAM_API_URL/genres/untagged-series"
```

Always run the audit on the targeted environment before proposing anything; the
untagged set changes as the indexer and metadata sync run.

## 4. Build the mapping

Read `references/genre-conventions.md` for the current taxonomy, the
neighbouring-series references, and the decision rules (manga vs BD vs comics,
Mature/Horror thresholds, Heroic Fantasy vs Fantasy).

For each untagged series:

1. Read the `description`, `authors`, `start_year` and book count.
2. Look up 2–3 already-tagged neighbouring series (`GET /series?q=...` or
   `GET /series?genre=...`) and mirror their genre sets.
3. Prefer 2–4 genres. One genre is acceptable for clearly single-axis works.
4. Use existing genre strings **exactly** (emoji + space included). A typo
   silently creates a new genre.
5. Flag series with `book_count = 0` and near-duplicate names as orphans instead
   of tagging them as if they were real series.

Present the mapping as a table: series, proposed genres, one-line justification,
plus a separate "orphans / duplicates" section. Wait for explicit validation
before any write.

## 5. Apply

One `POST /genres/assign` per genre, with the full batch of series:

```bash
curl -s -o /tmp/assign.json -w "%{http_code}\n" -X POST \
  -H "Authorization: Bearer $STRIPSTREAM_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"genre":"🧙 Fantasy","series_ids":["<uuid>","<uuid>"]}' \
  "$STRIPSTREAM_API_URL/genres/assign"
```

Rules:

- The endpoint **adds** a genre; it never removes the others. Re-running is
  idempotent.
- Fail if any response is not `200` and show the response body; do not silently
  continue.
- Never use `PATCH /series/{id}` with a partial array unless the user asks to
  replace genres — it overwrites the whole list.
- Do not invent genres. For a genuinely missing genre, ask first; a free tag can
  be created by assignment, but the taxonomy is curated (24–25 tags today).

## 6. Verify

```bash
# Must return []
curl -s -H "Authorization: Bearer $STRIPSTREAM_API_TOKEN" "$STRIPSTREAM_API_URL/genres/untagged-series"

# Spot-check 2–3 series
curl -s -H "Authorization: Bearer $STRIPSTREAM_API_TOKEN" "$STRIPSTREAM_API_URL/series/<uuid>/metadata"
```

Report: number of series tagged, genres applied per series, remaining untagged
count, and the orphan/duplicate list left for a separate decision.

## 7. Orphans and duplicates

Zero-book series and duplicate names are not genre problems. Report them with
their ids and let the user decide between deletion (`DELETE /series/{id}`) and
merge (`POST /series/{target}/merge` with the source id). Never delete or merge
without an explicit instruction naming the series.
