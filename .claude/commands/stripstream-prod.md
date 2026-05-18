---
name: "Stripstream: Production API"
description: Run operations against the production Stripstream API (genres, series, metadata)
category: Ops
tags: [ops, api, production]
---

Run an operation against the production Stripstream API.

**Credentials**: stored in `scripts/.env`
- `STRIPSTREAM_API_URL` = https://stripstream-api.julienfroidefond.com
- `STRIPSTREAM_API_TOKEN` = admin token (Bearer auth)

**Key endpoints**:
- `GET  /genres` — list all genres with series count
- `GET  /genres/untagged-series` — series with empty genres array
- `GET  /series/{id}/metadata` — SeriesMetadata: name, description, genres, authors, publishers, start_year, total_volumes, status
- `PATCH /series/{id}` — UpdateSeriesRequest: { new_name, genres, authors, publishers, description, start_year, total_volumes, status }
- `GET  /admin/series` — paginated list of all series (admin scope)
- `POST /genres/{name}/assign` — assign a genre to a series

**Auth header**: `Authorization: Bearer <token>`

**Scripts pattern**: write a `scripts/<name>.mjs` that imports credentials from `scripts/.env`, then run it with `node scripts/<name>.mjs`.

**Input**: Describe the operation to perform (e.g., "assign genres to untagged series", "rename genre X to Y", "list all series missing metadata").

Use `ANTHROPIC_API_KEY` from `process.env` for any Claude calls within the script (it's already in the shell environment).
