# AGENTS.md - Agent Coding Guidelines for Stripstream Librarian

Canonical guide for agentic coding agents. Read it first, then the module guide of every area
you touch: `apps/api/AGENTS.md`, `apps/indexer/AGENTS.md`, `apps/backoffice/AGENTS.md`,
`crates/parsers/AGENTS.md`.

Cargo workspace (`apps/api`, `apps/indexer`, `crates/core`, `crates/notifications`,
`crates/parsers`) plus three non-Rust apps (`apps/backoffice`, `apps/docs`). Companion docs:
`CLAUDE.md` (Claude Code specifics) and, on demand only, `.github/workflows/README.md`
(CI/deploy internals) and `.github/dependabot-policy.md` (dependency-update policy).

The official reader client lives in a separate repository,
[`julienfroidefond/stripstream`](https://github.com/julienfroidefond/stripstream), often cloned
as a sibling directory (`../stripstream`). It consumes the **read-scoped Client API**
(`/openapi.json` + `/swagger-ui`); treat that surface as a public contract and avoid breaking
changes.

---

## 1. Build, Lint, and Test Commands

```bash
cargo build                     # workspace
cargo build -p api              # single crate
cargo build --release
cargo watch -x build            # requires cargo-watch

cargo fmt --all -- --check      # or: cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --fix

cargo nextest run --workspace   # preferred (no doctests in this workspace)
cargo test --workspace          # equivalent fallback
cargo test -p api -- series::tests     # single module
cargo test test_name -- --nocapture    # single test, show stdout

# DB integration tests (PostgreSQL on localhost:6432)
DATABASE_URL="postgres://stripstream:stripstream@localhost:6432/stripstream" cargo nextest run --workspace
```

- **Tests are mandatory.** Any code change MUST ship with tests. Always run them before committing.
- `#[sqlx::test(migrations = "../../infra/migrations")]` creates a throwaway database per test with
  migrations applied. The PostgreSQL user needs the `CREATEDB` right:
  `ALTER USER stripstream CREATEDB;`.
- **Migrations**: `sqlx migrate run` and `sqlx migrate add -r migration_name` (requires
  `DATABASE_URL`). Files live in `infra/migrations/`. Always migrate before starting the services.

### Local stack

`docker-compose.yml` is at the repository root (not in `infra/`).

```bash
docker compose up -d postgres   # PostgreSQL only
docker compose up -d            # full stack
docker compose logs -f api
docker compose logs -f indexer

cd apps/backoffice && npm install && npm run dev      # http://localhost:7082
cd apps/docs && npm install && npm run dev -- --port 7083
./scripts/docker-push.sh        # local build & push, rarely needed
```

| Service | Folder | Port | Stack |
|---------|--------|------|-------|
| REST API | `apps/api/` | 7080 | axum |
| Indexer (background) | `apps/indexer/` | 7081 | axum (minimal) |
| Backoffice | `apps/backoffice/` | 7082 | Next.js 16 / React 19 |
| Documentation | `apps/docs/` | 7084 (prod), 7083 (dev) | Astro / Starlight |
| PostgreSQL | `infra/` | 6432 (host) → 5432 (container) | postgres:16-alpine |

---

## 2. Code Style Guidelines

### General Principles

- **Conciseness**: keep responses short and direct. Avoid unnecessary preamble.
- **Idiomatic Rust**: follow Rust best practices and ecosystem conventions.
- **Error Handling**: `anyhow::Result<T>` + `with_context()` for application code;
  `Result<T, ApiError>` in API handlers.
- **Async**: use `tokio`. Prefer `#[tokio::main]` over manual runtime setup.
- **Factor out, split up**: always extract duplicated logic into reusable functions/modules. When a
  file gets too long, split it into smaller cohesive modules. Refactor oversized existing files
  proactively.
- **User docs**: when a change affects visible behavior, admin workflows, notifications, settings,
  or user-facing job output, update the relevant docs in `apps/docs/` in the same change unless
  explicitly told not to.
- **Imports**: **std → external crates → workspace crates → local (`crate::`)**, blank line between
  groups. External crates are declared once in the **root** `Cargo.toml`
  (`[workspace.dependencies]`), never in individual crate manifests.
- **Errors**: `ApiError::bad_request()`, `not_found()`, `internal()`, `unauthorized()`,
  `forbidden()` live in `apps/api/src/error.rs`; `sqlx::Error` and `std::io::Error` convert
  automatically. Use `?` everywhere; never `unwrap()` in production paths.
- **Database (sqlx)**: raw `sqlx::query()` / `query_scalar()`, no ORM; **always** parameterized
  (`$1`, `$2`, ...), never string interpolation; bulk inserts/updates via `UNNEST` (see
  `apps/indexer/src/batch.rs`); multi-statement work in a transaction (`pool.begin()` /
  `tx.commit()`).
- **Async / Tokio**: `spawn_blocking` is mandatory for CPU-bound work (page rendering, opening
  archives, thumbnail generation); `tokio::spawn` for background tasks; bound concurrency with a
  `Semaphore` (page rendering) or `for_each_concurrent` (analysis); `tokio::time::timeout` for
  operations that can hang.
- **Structs & serialization**: `#[derive(Serialize, Deserialize, ToSchema)]` for exposed API types;
  `utoipa` for OpenAPI; `Option<T>` for nullable fields, public structs documented briefly.
- **Logging (`RUST_LOG`)**: targets `indexer`, `scan`, `extraction`, `thumbnail`, `watcher`.
  Default: `indexer=info,scan=info,extraction=info,thumbnail=warn,watcher=info`.
- **Performance**: batch DB writes (100 items recommended); `rayon::par_iter()` for CPU-intensive
  scans; cache expensive operations (see `apps/api/src/books/pages.rs` for the memory-LRU + disk
  cache); stream large data.

### Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Variables | snake_case | `let book_id = ...` |
| Functions | snake_case | `fn get_book(...)` |
| Structs/Enums | PascalCase | `struct BookItem` |
| Modules | snake_case | `mod books;` |
| Constants | SCREAMING_SNAKE_CASE | `const BATCH_SIZE: usize = 100;` |
| Types | PascalCase | `type MyResult<T> = Result<T, Error>;` |

### Testing

Write tests as you write code — this is not optional. Two kinds:

- **Unit tests (`#[test]`)** for pure logic: parsing, volume extraction, title matching, accent
  normalization, format dedup.
- **DB integration tests (`#[sqlx::test]`)** for real SQL — each test gets its own migrated temp
  database. Test modules are colocated (`apps/api/src/tests/`, `apps/api/src/series/tests/`, ...).

What to test: pure functions (volume parsing, title matching, accent normalization, format dedup),
SQL queries (case/accent-insensitive matching, upserts, lateral joins), and business logic
(torrent import, series creation dedup, metadata providers).

---

## 3. Project Structure

```
apps/api/        # REST API (axum) — 7080: books/, metadata/, metadata_providers/,
                 # integrations/, downloads/, jobs/, reading/, series/, reading_lists/, users/
apps/indexer/    # background indexing — 7081: worker, scanner, analyzer, batch, job,
                 # scheduler, watcher, converter, utils
apps/backoffice/ # Next.js 16 admin UI — 7082
apps/docs/       # Astro/Starlight docs site — 7084
crates/core/     # config, paths, fingerprint, schedule, heartbeat, disk
crates/parsers/  # CBZ/CBR/PDF/EPUB parsing + matching
crates/notifications/  # Telegram notifications
infra/migrations/      # SQL migrations (sqlx)
data/thumbnails/       # thumbnails generated by the indexer
libraries/             # book storage (mounted volume)
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
| `apps/indexer/src/scanner.rs` | Phase 1 discovery: fast scan, no archive I/O |
| `apps/indexer/src/analyzer.rs` | Phase 2 analysis: `analyze_book` + WebP thumbnails |
| `apps/indexer/src/batch.rs` | Bulk DB ops via `UNNEST` |
| `apps/indexer/src/worker.rs` | Job loop, watcher, scheduler orchestration |
| `apps/indexer/src/converter.rs` | CBR → CBZ conversion |
| `crates/parsers/src/lib.rs` | Format detection, metadata parsing, page extraction |
| `crates/core/src/config.rs` | Configuration from environment |
| `infra/migrations/*.sql` | Database schema |

---

## 4. Common Patterns

### Configuration from Environment

`*Config::from_env()` in `crates/core/src/config.rs` builds the config with `std::env::var`,
using `.context("... is required")` for mandatory values. Required variables: `DATABASE_URL`,
`API_BOOTSTRAP_TOKEN`, `ADMIN_USERNAME`, `ADMIN_PASSWORD`, `SESSION_SECRET` (min 32 chars). Full
table: `apps/docs/src/content/docs/reference/environment.md`.

### Metadata Sync (`apps/api/src/metadata/shared_sync.rs`)

All three sync paths (approve, batch auto-match, refresh) share factored code:
`extract_series_fields()`, `upsert_series_metadata()`, `fetch_local_books()` + `match_books()`,
`push_book_metadata()`, plus diff helpers. All providers must store `description` inside
`metadata_json` (not only the struct field) so it survives the DB round-trip.

### Metadata Providers

`available_providers()`: `google_books`, `open_library`, `comicvine`, `anilist`, `bdtheque`,
`bdphile`, `senscritique`. All implement `MetadataProvider` (`search_series` + `get_series_books`).
SensCritique uses a GraphQL API; BDTheque/BDphile scrape HTML. `bedetheque` was removed (Cloudflare
blocking) — migration `0114` unlinks existing links. Discovery sources backed by SensCritique
(`senscritique_bd`, `sc_trending_bd`, `sc_trending_manga`, `sc_best_bd`, `sc_best_manga`) are
**not** metadata providers: `metadata_providers::canonical_provider_name()` maps them to
`senscritique`. `metadata_providers::resolve_provider()` is the single chokepoint used by search,
batch and refresh to canonicalize then reject unknown names with an explicit error (never a silent
`google_books` fallback). The legacy link-sync path (`sync::resolve_link_provider_name`) also
canonicalizes, but keeps the historic `google_books` fallback for links that were created outside
the metadata providers (e.g. Prowlarr downloads). Migration `0129` heals legacy links that stored an
alias.

### Path Remapping & Indexer Pipeline

Path remapping (`remap_libraries_path` / `unmap_libraries_path`, `crates/core/src/paths.rs` /
`apps/indexer/src/utils.rs`) and the 2-phase indexer architecture (discovery then analysis) are
documented in `apps/indexer/AGENTS.md`.

---

## 5. Gotchas

- **`volume_type`** (`regular`, `hs`, `oneshot`, `integral`): all metadata matching, missing-volume
  counting, and AniList progress logic MUST filter on `volume_type = 'regular'`. HS/oneshot/integral
  do not participate in tome numbering.
- **Series matching**: always compare with `LOWER(unaccent(name))`. Never match exactly — torrents,
  providers, and the UI use different casing/accents.
- **Auth tokens**: format `stl_<prefix>_<secret>`, argon2 hash in DB, scopes `admin` or `read`.
- **OpenAPI dual spec**: Client API (`/openapi.json`, read scope) and Admin API
  (`/admin/openapi.json`, all scopes); UIs at `/swagger-ui` and `/admin/swagger-ui`.
  `GET /metadata/links` and `GET /metadata/missing/:id` are on the **read** router
  (`apps/api/src/main.rs`), so they only require a `read`-scoped token.
- **Search**: PostgreSQL full-text (`ILIKE` + `pg_trgm`) — no external search engine.
- **Stale pending jobs**: jobs `pending` for > 30 min are marked `failed` by cleanup (otherwise they
  block the scheduler's `NOT EXISTS` and prevent future scheduled jobs).
- **Docs (Astro/Starlight)**: static site in `apps/docs/`, `npm run build`, served by nginx in
  Docker. Markdown content in `src/content/docs/`, sidebar in `astro.config.mjs`, custom
  cyan/magenta theme in `src/styles/custom.css`.
- **Database**: PostgreSQL is required. Run migrations before starting the services.
