# AGENTS.md - Agent Coding Guidelines for Stripstream Librarian

Canonical guide for agentic coding agents. Read it first, then the module guide of every area
you touch: `apps/api/AGENTS.md`, `apps/indexer/AGENTS.md`, `apps/backoffice/AGENTS.md`,
`crates/parsers/AGENTS.md`.

Cargo workspace (`apps/api`, `apps/indexer`, `crates/core`, `crates/jobs`,
`crates/notifications`, `crates/parsers`, `crates/thumbnails`) plus three non-Rust apps (`apps/backoffice`, `apps/docs`). Companion docs:
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
- **Rust build cache**: `target/` grows to several GB per worktree. The dev profile keeps only
  line tables (`debug = "line-tables-only"`) and no dependency debug info. When disk fills up, run
  `./scripts/prune-rust-target.sh --apply` to drop the `incremental` caches of the repo and all its
  git worktrees; add `--stale-days 30` to also prune old artifacts with `cargo-sweep`.
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
- **Performance**: batch DB writes (100 items recommended); bound CPU-intensive
  work with `spawn_blocking` + a concurrency limit; cache expensive operations (see `apps/api/src/books/renderer.rs` for the memory-LRU + disk
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
crates/core/     # config, paths, fingerprint, schedule, heartbeat, disk, limits
crates/parsers/  # CBZ/CBR/PDF/EPUB parsing + matching
crates/notifications/  # Telegram notifications
crates/thumbnails/     # shared thumbnail store (settings, encode, atomic publish, purge)
infra/migrations/      # SQL migrations (sqlx)
data/thumbnails/       # thumbnails generated by the indexer
libraries/             # book storage (mounted volume)
```

### Key Files

| File | Purpose |
|------|---------|
| `apps/api/src/main.rs` | Router, AppState init, Swagger UI (client + admin) |
| `apps/api/src/state.rs` | `AppState`: pool, caches, metrics, resizable `page_render_limit` |
| `crates/core/src/limits.rs` | Typed `app_settings.limits` schema shared by API and indexer (`Limits::parse` / `load`, ADR 0026) |
| `apps/api/src/error.rs` | `ApiError` + constructors |
| `apps/api/src/books/renderer.rs` | `PageRenderer`: the one page-rendering pipeline + double cache (memory LRU + disk), ADR 0023 |
| `apps/api/src/books/files.rs` | `current_book_file` / `current_book_file_join!`: the current file of a book |
| `apps/api/src/books/thumbnails.rs` | Thumbnail rebuild/regenerate job endpoints |
| `apps/api/src/metadata/shared_sync.rs` | Factored sync used by approve + batch + refresh |
| `apps/api/src/metadata_providers/mod.rs` | `MetadataProvider` trait + `available_providers()` |
| `apps/indexer/src/scanner.rs` | Phase 1 discovery: fast scan, no archive I/O |
| `apps/indexer/src/analyzer.rs` | Phase 2 analysis: `analyze_book` + thumbnails via the shared store |
| `crates/thumbnails/src/lib.rs` | `ThumbnailSettings` / `ThumbnailStore` shared by API and indexer |
| `crates/notifications/src/job.rs` | `notify_job`: the notification policy of a finished job (ADR 0032) |
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

### Thumbnail Store (`crates/thumbnails`)

`ThumbnailSettings::load(pool)` is the only reader of the thumbnail configuration: the
`thumbnail` row of `app_settings`, field by field, over the built-in defaults (`webp`, 300×400,
quality 80, `/data/thumbnails`). No `THUMBNAIL_*` environment variable is read. `ThumbnailStore`
owns the `<directory>/<book_id>.<webp|jpg|png>` layout (`path_for`, `book_id_of`), `encode`,
atomic `publish` (temp file + rename, drops the other formats), `stage_raw` / `publish_raw`
(indexer `.raw` pages), `purge` and `purge_orphans`. `fast_jpeg_decode` is the shared DCT-scaled
decoder. Never write, name or delete a thumbnail file outside this crate.

### Metadata Sync (`apps/api/src/metadata/shared_sync.rs`)

All three sync paths (approve, batch auto-match, refresh) share factored code:
`extract_series_fields()`, `upsert_series_metadata()`, `fetch_local_books()` + `match_books()`,
`push_book_metadata()`, plus diff helpers. All providers must store `description` inside
`metadata_json` (not only the struct field) so it survives the DB round-trip.

### Volume Coverage (`crates/core/src/coverage.rs`)

`SeriesCoverage` is the single source of truth for tome ownership, completeness and missing
counts. Numbered `regular`/`integral` volumes (`volume >= 1`) cover their own tome; an
unnumbered `regular`/`integral` covers one unspecified tome; an unnumbered `integral` completes
the whole series; `hs`, `oneshot` and volume `0` never count. Database adapters live in
`apps/api/src/coverage.rs` (`missing_counts_cte_for_library`, `missing_counts_cte_for_series`,
`load_series_coverages`, `load_owned_volumes_for_series_name`); the indexer uses the same core
type in `cleanup_available_downloads`. Never re-encode this rule in a handler or a one-off SQL
query.

### Download Availability (`crates/core/src/availability.rs`)

`available_downloads` is written from two sides: detection/RSS/DDL **merge** releases in, and
scan cleanup, torrent import and detection normalization **prune** volumes out. Both directions
and the `missing_count` derivation live in the core module. `PresentVolumes` wraps a
`SeriesCoverage` (or imported volumes); `prune_releases` / `normalize_releases` own the release
JSON walk; `merge_releases`, `prune_series`, `prune_library` and `normalize_library` own the SQL.
The counter decrements by the number of **distinct volumes actually removed** from at least one
release — never by the number of imported files or release matches. Do not re-implement the
release JSON filtering in the API or the indexer.

### Release Ingestion (`apps/api/src/downloads/ingest.rs`, ADR 0031)

`store_releases` is the only way a release enters `available_downloads`: it drops blacklisted
titles, stamps `detected_at`, emits the source's "found" event, merges through
`availability::merge_releases` and returns `Stored::Nothing` / `Found(n)`. Detection, the RSS poll
and the DDL search keep their own collection loops but share this publication step. What differs
between sources is data: a `StorePolicy` (event name, `EntityKind`, `ReleaseDetail`, `MergeFailure`)
built by `prowlarr_detection()`, `prowlarr_rss()` or `ddl_catalogue()`. `new_releases_since`,
`new_items_since`, `event_counts` and `cleanup` own the run-level reads and cleanup. Never write the
found event, the `detected_at` stamp or the merge inline in a download flow.

### Job Notifications (`crates/notifications/src/job.rs`, ADR 0032)

`notify_job(pool, JobFinished)` is the only way a finished indexer job becomes Telegram messages.
`JobFinished` says what happened (`job_id`, `library_id`, `JobOutcome`, duration); the module loads
the job, its library, its book, `stats_json` and the scan's `book_added` events. `job_events` is the
DB seam, `events_for` the pure policy (which events, and when the scan summary is worth sending) and
`deliver` the shared config gate, formatting and send (`notify` uses it too). Never load
`stats_json` or `book_added` in the indexer to build an event. `stats_json` has one schema:
`stripstream_jobs::ScanStats` (serde), which `scanner::JobStats` aliases. The worker only calls
`notify_job`; `cleanup_available_downloads` stays in the indexer.

### Series Catalogue Read Model (`apps/api/src/series/catalog/`, ADR 0030)

`catalog::query(pool, SeriesRequest)` is the only read that projects a `SeriesItem`: `/series`,
`/libraries/{id}/series`, `/favorites`, `/series/ongoing` and `/series/{id}/details` all go through
it. A `SeriesScope` owns the staging CTEs, the `series_counts` body, the order and the predicate the
missing count aggregates on; filters, shelf/rating/progress joins and the row projection are shared.
Handlers translate their DTO into a typed `SeriesRequest` and only shape the response. Never add a
query that projects `SeriesItem` elsewhere, and never silently ignore an unknown filter value — it is
a 400 listing the accepted values.

### Series Identity (`crates/core/src/series_identity.rs`)

The SQL `norm_text` key (lowercase + unaccent) is the only series-name key. A raw name matches a
series whose `norm_text(name)` **or** `norm_text(original_name)` equals it (a current-name match
wins), so a renamed series keeps owning its folder name. `find` / `find_id`, `find_many`,
`resolve_many` and `existing_names` own point and batch lookups; all but `existing_names` are
library-scoped. `find_or_create` is the only get-or-create: it serializes creation per normalized
name with an advisory lock (ADR 0022) — never write `INSERT INTO series ... ON CONFLICT (library_id,
name)` to resolve a name. Callers pass **raw names** and get series rows back — never hand-write
`LOWER(name)` or a new `norm_text` join. Set-based joins that already use `norm_text` stay as-is;
any new lookup must go through this module.

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

Match scores live in `metadata_providers/scoring.rs` (ADR 0015): providers build a `Score` with
`Score::of_titles(title, query, cap)` (one normalized `title_similarity`), the book-count bonus is
added before the cap (`apply_book_count`) and `Score::is_auto_match()` is the only auto-approval
verdict. Never compute a confidence or compare it to `1.0` by hand.

### Path Remapping & Indexer Pipeline

Path handling uses typed `LibraryRoot` / `LibraryPath` values from `crates/core/src/paths.rs`
(`resolve`, `to_local`, `to_db`) — never join `/libraries` by hand. The 2-phase indexer
architecture (discovery then analysis) and the `books.analysis_status` lifecycle are documented in
`apps/indexer/AGENTS.md`. Every `analysis_status` transition (claim, release, failure, page counts,
covers from the indexer or the API, cover reset) goes through `crates/core/src/analysis.rs`;
never write that column in SQL elsewhere (discovery only inserts `discovered`).

---

## 5. Gotchas

- **`volume_type`** (`regular`, `hs`, `oneshot`, `integral`): all metadata matching, missing-volume
  counting, and AniList progress logic MUST filter on `volume_type = 'regular'`. HS/oneshot/integral
  do not participate in tome numbering. Exception: completion/progress *counts* sent to AniList
  (`reading/status_push.rs`, `integrations/anilist_sync.rs` stats) use `IN ('regular', 'integral')`
  — an integral makes a series complete (see `apps/docs/.../integrations/anilist.md`).
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
- **Changelog**: `CHANGELOG.md` (repo root) and `apps/docs/.../reference/changelog.md` are
  generated by `scripts/generate-changelog.sh` from the `v*` tags and conventional commits —
  never edit them by hand. The deploy workflow regenerates both inside the version bump.
- **Database**: PostgreSQL is required. Run migrations before starting the services.
