# AGENTS.md - Agent Coding Guidelines for Stripstream Librarian

This file is the canonical guide for agentic coding agents operating in this repository.
Read it first. Companion docs:

- `CLAUDE.md` — Claude Code specifics (environment variables, detailed gotchas).
- Per-module guides: `apps/api/AGENTS.md`, `apps/indexer/AGENTS.md`, `apps/backoffice/AGENTS.md`, `crates/parsers/AGENTS.md`.

The workspace is a Cargo workspace (`apps/api`, `apps/indexer`, `crates/core`, `crates/notifications`, `crates/parsers`) plus three non-Rust apps (`apps/backoffice`, `apps/docs`).

---

## 1. Build, Lint, and Test Commands

### Build

```bash
# Whole workspace
cargo build

# Single crate
cargo build -p api
cargo build -p indexer

# Release
cargo build --release

# Watch mode (requires cargo-watch)
cargo watch -x build
```

### Lint & Format

```bash
cargo clippy
cargo clippy --fix
cargo fmt
cargo fmt -- --check
```

### Tests

**Tests are mandatory.** Any code change MUST ship with tests. Always run them before committing.

```bash
# Unit tests (no database required)
cargo test --workspace
cargo test -p api
cargo test -p api -- series::tests     # single module
cargo test test_name                   # single test by name
cargo test -- --nocapture              # show stdout
cargo test --doc

# DB integration tests (require PostgreSQL on localhost:6432)
DATABASE_URL="postgres://stripstream:stripstream@localhost:6432/stripstream" cargo test --workspace
```

`#[sqlx::test(migrations = "../../infra/migrations")]` creates a throwaway database per test with
migrations applied. The PostgreSQL user needs the `CREATEDB` right: `ALTER USER stripstream CREATEDB;`.

### Database Migrations

```bash
# Requires DATABASE_URL to be set
sqlx migrate run
sqlx migrate add -r migration_name   # creates up + down files
```

Migrations live in `infra/migrations/`. Always migrate before starting the services.

### Docker & Local Dev Servers

`docker-compose.yml` is at the repository root (not in `infra/`).

```bash
# Start PostgreSQL only
docker compose up -d postgres

# Full stack
docker compose up -d
docker compose logs -f api
docker compose logs -f indexer

# Backoffice (Next.js) — http://localhost:7082
cd apps/backoffice && npm install && npm run dev

# Docs (Astro/Starlight) — dev server on 7083
cd apps/docs && npm install && npm run dev -- --port 7083

# Build & push images (interactive: bump version, select services)
./scripts/docker-push.sh
```

### Services & Ports

| Service | Folder | Port | Stack |
|---------|--------|------|-------|
| REST API | `apps/api/` | 7080 | axum |
| Indexer (background) | `apps/indexer/` | 7081 | axum (minimal) |
| Backoffice | `apps/backoffice/` | 7082 | Next.js 16 / React 19 |
| Documentation | `apps/docs/` | 7084 (prod), 7083 (dev) | Astro / Starlight |
| PostgreSQL | `infra/` | 6432 (host) → 5432 (container) | postgres:16-alpine |

The API and indexer share a single Dockerfile (`apps/api/Dockerfile`) with two targets
(`--target api` / `--target indexer`). Build uses **cargo-chef** (planner → recipe.json → cooked deps
layer), the **mold** linker, and a separate `sqlx-installer` stage so app rebuilds never recompile
`sqlx-cli`. CI lives in two places. Gitea Actions (`.gitea/workflows/deploy.yml`): per-service
change detection (`git diff HEAD~1`), conditional build, registry cache, automatic deploy.
A GitHub Actions mirror (`/.github/workflows/`) runs the same change detection plus
`cargo fmt` / `clippy` / `test` (`.github/workflows/ci.yml`) and builds & pushes the Docker
images, but omits the local stack restart (GitHub-hosted runners cannot reach the deploy host).
Web changes (`apps/backoffice`, `apps/docs`) are additionally validated on pull requests
(`.github/workflows/ci-web.yml`).

---

## 2. Code Style Guidelines

### General Principles

- **Conciseness**: keep responses short and direct. Avoid unnecessary preamble.
- **Idiomatic Rust**: follow Rust best practices and ecosystem conventions.
- **Error Handling**: `anyhow::Result<T>` + `with_context()` for application code; `Result<T, ApiError>` in API handlers.
- **Async**: use `tokio`. Prefer `#[tokio::main]` over manual runtime setup.
- **Factor out, split up**: always extract duplicated logic into reusable functions/modules. When a file
  gets too long, split it into smaller cohesive modules. Refactor oversized existing files proactively.
- **User docs**: when a change affects visible behavior, admin workflows, notifications, settings, or
  user-facing job output, update the relevant docs in `apps/docs/` in the same change unless explicitly told not to.

### Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Variables | snake_case | `let book_id = ...` |
| Functions | snake_case | `fn get_book(...)` |
| Structs/Enums | PascalCase | `struct BookItem` |
| Modules | snake_case | `mod books;` |
| Constants | SCREAMING_SNAKE_CASE | `const BATCH_SIZE: usize = 100;` |
| Types | PascalCase | `type MyResult<T> = Result<T, Error>;` |

### Imports

Order: **std → external crates → workspace crates → local (`crate::`)**, with blank lines between groups.

```rust
use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use parsers::detect_format;

use crate::error::ApiError;
use crate::AppState;
```

External crates are declared once in the **root** `Cargo.toml` (`[workspace.dependencies]`), never in
individual crate manifests.

### Error Handling

- Use `anyhow` + `with_context()` for application code.
- Return `Result<T, ApiError>` in API handlers. Constructors (`apps/api/src/error.rs`):
  `ApiError::bad_request()`, `not_found()`, `internal()`, `unauthorized()`, `forbidden()`.
  `sqlx::Error` and `std::io::Error` convert automatically.
- Use `?` everywhere; never `unwrap()` in production paths.

```rust
// Good
fn process_book(path: &Path) -> anyhow::Result<Book> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open file: {}", path.display()))?;
    // ...
}

// Good - API handler
async fn get_book(State(state): State<AppState>, Path(id): Path<Uuid>)
    -> Result<Json<Book>, ApiError> {
    let row = sqlx::query("SELECT * FROM books WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::internal)?;
    // ...
}
```

### Database (sqlx)

- Raw SQL with `sqlx::query()` / `sqlx::query_scalar()` — no ORM.
- **Always** parameterized queries (`$1`, `$2`, ...) — never string interpolation.
- Prefer **batch operations** using `UNNEST` for bulk inserts/updates (see `apps/indexer/src/batch.rs`).
- Transactions:

```rust
let mut tx = pool.begin().await?;
// ... queries ...
tx.commit().await?;
```

### Async / Tokio

- `spawn_blocking` is mandatory for CPU-bound work: page rendering, opening archives, thumbnail generation.
- `tokio::spawn` for background tasks.
- Bound concurrency with a `Semaphore` (page rendering) or `for_each_concurrent` (analysis).
- Use `tokio::time::timeout` for operations that can hang.

```rust
let bytes = tokio::time::timeout(
    Duration::from_secs(60),
    tokio::task::spawn_blocking(move || render_page(&abs_path_clone, n)),
)
.await
.map_err(|_| ApiError::internal("timeout"))?
.map_err(ApiError::internal)?;
```

### Structs & Serialization

- `#[derive(Serialize, Deserialize, ToSchema)]` for exposed API types.
- `utoipa` for OpenAPI. **Dual spec**: Client (`/openapi.json`, read scope) and Admin (`/admin/openapi.json`, all scopes);
  UIs at `/swagger-ui` and `/admin/swagger-ui` (see `apps/api/src/openapi.rs` — `ClientApiDoc` / `AdminApiDoc`).
- Use `Option<T>` for nullable fields and document public structs briefly.

```rust
#[derive(Serialize, ToSchema)]
pub struct BookItem {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub title: String,
    pub author: Option<String>,
    // ...
}
```

### Logging (`RUST_LOG`)

Domains (tracing targets): `indexer`, `scan`, `extraction`, `thumbnail`, `watcher`.
Levels: error, warn, info, debug, trace.
Default: `indexer=info,scan=info,extraction=info,thumbnail=warn,watcher=info`.

### Performance

- Batch DB writes (100 items recommended).
- `rayon::par_iter()` for CPU-intensive scans.
- Cache expensive operations (see `apps/api/src/books/pages.rs` for the memory-LRU + disk cache).
- Stream large data.

### Testing

Write tests as you write code — this is not optional. Two kinds:

**Unit tests (`#[test]`)** for pure logic: parsing, volume extraction, title matching, accent normalization.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn my_function_edge_case() {
        assert_eq!(my_function("input"), expected);
    }
}
```

**DB integration tests (`#[sqlx::test]`)** for real SQL — each test gets its own migrated temp database.
Test modules are colocated (`apps/api/src/tests/`, `apps/api/src/series/tests/`, ...).

```rust
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_case_insensitive(pool: sqlx::PgPool) {
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(lib_id).bind("test").bind("/libraries/test")
        .execute(&pool).await.unwrap();

    let id1 = get_or_create_series(&pool, lib_id, "Astérix").await.unwrap();
    let id2 = get_or_create_series(&pool, lib_id, "Asterix").await.unwrap();
    assert_eq!(id1, id2);
}
```

What to test: pure functions (volume parsing, title matching, accent normalization, format dedup),
SQL queries (case/accent-insensitive matching, upserts, lateral joins), and business logic
(torrent import, series creation dedup, metadata providers).

---

## 3. Project Structure

```
stripstream-librarian/
├── apps/
│   ├── api/               # REST API (axum) — port 7080
│   │   └── src/
│   │       ├── main.rs            # routes, AppState init, dual Swagger UI
│   │       ├── state.rs          # AppState (pool, caches, metrics), concurrent_renders
│   │       ├── error.rs          # ApiError constructors
│   │       ├── openapi.rs        # ClientApiDoc + AdminApiDoc
│   │       ├── api_middleware.rs # auth + rate limiting
│   │       ├── handlers.rs       # /health, /metrics
│   │       ├── books/            # mod.rs, pages.rs, thumbnails.rs, rename.rs
│   │       ├── metadata/         # shared_sync.rs, batch.rs, refresh.rs, handlers.rs, config.rs
│   │       ├── metadata_providers/  # google_books, open_library, comicvine, anilist,
│   │       │                        # bdtheque, bdphile, senscritique
│   │       ├── integrations/     # anilist*, komga, telegram, discovery/
│   │       ├── downloads/        # prowlarr, qbittorrent, torrent_import, telegram_monitor, rss_poll
│   │       ├── jobs/             # index_jobs.rs, poller.rs, helpers.rs
│   │       ├── reading/          # progress, status_match, status_pull, status_push
│   │       ├── series/           # list, create, merge, recommendations, ratings, archive...
│   │       ├── reading_lists/
│   │       ├── users/            # accounts, auth, tokens, genre_restrictions
│   │       └── tests/            # unit + DB tests
│   ├── indexer/           # Background indexing service — port 7081
│   │   └── src/          # worker.rs, scanner.rs, analyzer.rs, batch.rs,
│   │                     # job.rs, scheduler.rs, watcher.rs, converter.rs, utils.rs
│   ├── backoffice/        # Next.js 16 admin UI — port 7082
│   └── docs/              # Astro/Starlight documentation site — port 7084
├── crates/
│   ├── core/              # config.rs (env), paths.rs, fingerprint.rs, schedule.rs
│   ├── parsers/           # lib.rs (CBZ/CBR/PDF/EPUB parsing), matching.rs
│   ├── notifications/     # Telegram notifications
│   └── core2-shim/        # crates-io patch for `core2` (path override)
├── infra/
│   └── migrations/        # SQL migrations (sqlx)
├── docs/                  # FEATURES.md, KNOWN_ISSUES.md
├── data/thumbnails/       # Thumbnails generated by the indexer
├── libraries/             # Book storage (mounted volume)
└── docker-compose.yml     # At the repo root (not in infra/)
```

### Key Files

| File | Purpose |
|------|---------|
| `apps/api/src/main.rs` | Router, AppState init, Swagger UI (client + admin) |
| `apps/api/src/state.rs` | `AppState`: pool, caches, metrics, `page_render_limit` semaphore |
| `apps/api/src/error.rs` | `ApiError` + constructors |
| `apps/api/src/books/pages.rs` | Page rendering + double cache (memory LRU + disk) |
| `apps/api/src/books/thumbnails.rs` | Thumbnail rebuild/regenerate job endpoints |
| `apps/api/src/metadata/shared_sync.rs` | Factored sync used by approve + batch + refresh |
| `apps/api/src/metadata_providers/mod.rs` | `MetadataProvider` trait + `available_providers()` |
| `apps/indexer/src/scanner.rs` | Phase 1 discovery: fast scan, no archive I/O, skips unchanged dirs |
| `apps/indexer/src/analyzer.rs` | Phase 2 analysis: `analyze_book` + WebP thumbnail generation |
| `apps/indexer/src/batch.rs` | Bulk DB ops via `UNNEST` |
| `apps/indexer/src/worker.rs` | Job loop, watcher, scheduler orchestration |
| `apps/indexer/src/converter.rs` | CBR → CBZ conversion |
| `crates/parsers/src/lib.rs` | Format detection, metadata parsing, page extraction |
| `crates/core/src/config.rs` | Configuration from environment |
| `infra/migrations/*.sql` | Database schema |

---

## 4. Common Patterns

### Configuration from Environment

```rust
// crates/core/src/config.rs
impl IndexerConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: std::env::var("INDEXER_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:7081".to_string()),
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL is required")?,
            // ...
        })
    }
}
```

Required variables: `DATABASE_URL`, `API_BOOTSTRAP_TOKEN`, `ADMIN_USERNAME`, `ADMIN_PASSWORD`,
`SESSION_SECRET` (min 32 chars). See README for the full table.

### Path Remapping

Paths stored in the DB begin with `/libraries/` (container paths). On the host, `LIBRARIES_ROOT_PATH`
replaces `/libraries`. Use the helpers in `crates/core/src/paths.rs` / `apps/indexer/src/utils.rs`:

```rust
remap_libraries_path(&abs_path)    // DB path → local filesystem
unmap_libraries_path(&local_path)  // local filesystem → DB path
```

### Metadata Sync (`apps/api/src/metadata/shared_sync.rs`)

All three sync paths (approve, batch auto-match, refresh) share factored code:
`extract_series_fields()`, `upsert_series_metadata()`, `fetch_local_books()` + `match_books()`,
`push_book_metadata()`, plus diff helpers. All providers must store `description` inside
`metadata_json` (not only the struct field) so it survives the DB round-trip.

### Metadata Providers

`available_providers()`: `google_books`, `open_library`, `comicvine`, `anilist`, `bdtheque`,
`bdphile`, `senscritique`. All implement `MetadataProvider` (`search_series` + `get_series_books`).
SensCritique uses a GraphQL API; BDTheque/BDphile scrape HTML. `bedetheque` was removed (Cloudflare
blocking) — migration `0114` unlinks existing links. Discovery providers (`sc_trending_bd`,
`sc_best_manga`, ...) are normalized to the provider name `senscritique` when creating metadata links.

### Indexer 2-Phase Pipeline

1. **Discovery** (`scanner.rs`) — `WalkDir` + filename-only metadata (zero archive I/O). Inserts books
   with `page_count = NULL` so they are visible immediately. Skips unchanged directories via the
   `directory_mtimes` table.
2. **Analysis** (`analyzer.rs`) — opens archives, extracts page count + first page, generates WebP
   thumbnails. Processes books where `page_count IS NULL` (or `thumbnail_path IS NULL` for thumbnail jobs).

Fingerprint = `SHA256(size + mtime + filename)` — detects changes without re-reading files.

---

## 5. Gotchas

- **No external CLI tools are invoked.** CBR page listing/extraction runs in-process via the `unrar`
  crate; PDF page count + render use `pdfium-render`, which binds `libpdfium.so` at runtime
  (`Pdfium::bind_to_system_library()`). The Docker runtime stage installs libpdfium from
  `pdfium-binaries`. `unar`, `pdfinfo` and `pdftoppm` are **not** used.
- **`page_count = NULL`** is normal after phase 1 (discovery). Phase 2 (analysis) fills it. Do not treat as an error.
- **Thumbnails** are generated by the **indexer** (phase 2, `analyzer.rs`), not the API. The API only creates DB jobs.
- **`volume_type`** (`regular`, `hs`, `oneshot`, `integral`): all metadata matching, missing-volume
  counting, and AniList progress logic MUST filter on `volume_type = 'regular'`. HS/oneshot/integral
  do not participate in tome numbering.
- **Scanner propagates `volume_type`**: on both skipped-directory and fingerprint-unchanged updates, the
  scanner re-compares and rewrites `books.volume_type` (`apps/indexer/src/scanner.rs`). A plain rescan is
  enough to fix misclassified HS/oneshot/integral books.
- **Series matching**: always compare with `LOWER(unaccent(name))`. Never match exactly — torrents,
  providers, and the UI use different casing/accents.
- **Series extraction**: the parser uses the **immediate parent** directory as the series name (not the
  first directory). If the parent is an HS/Specials/Bonus/Intégrales subfolder, it walks up one level.
- **Auth tokens**: format `stl_<prefix>_<secret>`, argon2 hash in DB, scopes `admin` or `read`.
- **OpenAPI dual spec**: Client API (`/openapi.json`, read scope) and Admin API (`/admin/openapi.json`, all).
  `GET /metadata/links` and `GET /metadata/missing/:id` are on the **read** router (`apps/api/src/main.rs`),
  so they only require a `read`-scoped token.
- **Search**: PostgreSQL full-text (`ILIKE` + `pg_trgm`) — no external search engine.
- **Stale pending jobs**: jobs `pending` for > 30 min are marked `failed` by cleanup (otherwise they block
  the scheduler's `NOT EXISTS` and prevent future scheduled jobs).
- **Torrent import replace mode**: when `replace_existing = true`, do NOT filter by `expected_volumes` —
  import every file in the torrent.
- **Import counts**: `ImportedFile.already_existed` (`apps/api/src/downloads/torrent_import.rs`) separates
  files actually copied from those already on disk. Counts and notifications only include real new files.
- **Duplicate torrent detection**: when an existing torrent is found by magnet hash, verify `content_path`
  exists on disk before starting the import. Old `sl-*` directories are cleaned up after import.
- **Missing books dedup**: `external_book_metadata` can contain duplicates per `volume_number` (multiple
  editions). Display queries use `DISTINCT ON (volume_number)`.
- **Backoffice (Next.js)**: prefer Server Components; use `"use client"` only for necessary interactivity.
  All API calls live in `lib/api.ts`; reuse generic components from `app/components/ui/` instead of raw HTML.
  `staleTimes.dynamic = 30`; API fetches use `{ next: { revalidate: N } }` (15s lists, 30-60s stable data).
  Images use `unoptimized: true` — serve pre-optimized WebP from `public/`.
- **Next.js 16 production**: `router.replace()` does not work in standalone mode. Use
  `window.history.replaceState()` + `router.refresh()` (see `LiveSearchForm.tsx`).
- **Docs (Astro/Starlight)**: static site in `apps/docs/`, `npm run build`, served by nginx in Docker.
  Markdown content in `src/content/docs/`, sidebar in `astro.config.mjs`, custom cyan/magenta theme in
  `src/styles/custom.css`.
- **Workspace**: external dependencies are defined in the root `Cargo.toml`, not in individual crates.
- **Database**: PostgreSQL is required. Run migrations before starting the services.
