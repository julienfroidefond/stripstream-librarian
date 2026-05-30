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

**Approach**: Make API calls directly with `curl` via Bash — do NOT write scripts. Source credentials with `source scripts/.env` before each curl block.

**Input**: Describe the operation to perform (e.g., "assign genres to untagged series", "rename genre X to Y", "list all series missing metadata").

**Notes**:
- `GET /series?genre=<name>` filters series by genre (useful to find which series uses a genre before renaming it)
- To rename a genre: find the series using it, then `PATCH /series/{id}` with the updated genres array — the old genre disappears automatically when no series reference it anymore
- `GET /admin/series` returns 404 — do not use it
