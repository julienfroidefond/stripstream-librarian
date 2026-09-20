# CLAUDE.md

Guidance for Claude Code when working in this repository.

> **Canonical documentation lives in [`AGENTS.md`](./AGENTS.md).**
> That file is the single source of truth for build/lint/test commands, code style, project structure,
> common patterns and gotchas — it is auto-loaded alongside this one.
>
> **Do not duplicate `AGENTS.md` here.** Update `AGENTS.md` instead. This file deliberately keeps only
> what is specific to Claude Code.

## Per-module guides

- `apps/api/AGENTS.md` — REST API (axum)
- `apps/indexer/AGENTS.md` — background indexer
- `apps/backoffice/AGENTS.md` — Next.js admin UI
- `crates/parsers/AGENTS.md` — CBZ/CBR/PDF/EPUB parsing

## Claude Code specifics

### Repo-local Claude config

| Path | Purpose |
|------|---------|
| `.claude/commands/` | Project slash commands |
| `.claude/skills/` | Project skills |
| `.claude/settings.local.json` | Local permission settings (not shared with other agents) |

### Bootstrap

```bash
cp .env.example .env   # then edit the REQUIRED values
```

Required: `DATABASE_URL`, `API_BOOTSTRAP_TOKEN`, `ADMIN_USERNAME`, `ADMIN_PASSWORD`,
`SESSION_SECRET` (min 32 chars). Full variable table lives in `AGENTS.md` / `README.md`.

DB-backed tests need PostgreSQL on `localhost:6432` and a user with the `CREATEDB` right.
