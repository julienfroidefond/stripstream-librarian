# Stripstream Librarian — Features & Business Rules

## Libraries

### Multi-Library Management
- Create and manage multiple independent libraries, each with its own root path
- Enable/disable libraries individually
- Delete a library cascades to all its books, jobs, and metadata

### Scanning & Indexing
- **Incremental scan**: uses directory mtime tracking to skip unchanged directories
- **Full rebuild**: force re-walk all directories, ignoring cached mtimes
- **Rescan**: deep rescan to discover newly supported formats
- **Two-phase pipeline**:
  - Phase 1 (Discovery): fast filename-based metadata extraction (no archive I/O)
  - Phase 2 (Analysis): extract page counts, first page image from archives

### Real-Time Monitoring
- **Automatic periodic scanning**: configurable interval (default 5 seconds)
- **Filesystem watcher**: real-time detection of file changes for instant indexing
- Each can be toggled per library (`monitor_enabled`, `watcher_enabled`)

---

## Books

### Format Support
- **CBZ** (ZIP-based comic archives)
- **CBR** (RAR-based comic archives)
- **PDF**
- **EPUB**
- Automatic format detection from file extension and magic bytes

### Metadata Extraction
- **Title**: derived from filename or external metadata
- **Series**: derived from directory structure (immediate parent directory of the file). If the parent is an HS/special subfolder (Hors-Série, Specials, Bonus, Extras, HS, Intégrales, INT), uses the grandparent instead. Supports nested structures (e.g., `Shonen/Dragon Ball/T01.cbz` → series = "Dragon Ball").
- **Volume**: extracted from filename with pattern detection:
  - `Tome ##`, `Tome.##` — French comics (priorité haute)
  - `T##` (Tome abrégé) — most common for French comics
  - `Vol.##`, `Vol ##`, `Volume ##`
  - `###` (hash prefix)
  - `-## ` (dash-separated at end)
  - `Tome_##` — underscore separator
- **Volume type** (`volume_type`): classification of each book
  - `regular` — standard numbered volume (default)
  - `hs` — hors-série / special edition (detected: HS, Hors-Série, Spécial, Bonus)
  - `integral` — omnibus / intégrale (detected: INT, INTHS, Intégrale)
  - `oneshot` — standalone book (set manually)
- **Author(s)**: single scalar and array support
- **Page count**: extracted from archive analysis
- **Language**, **kind** (ebook, comic, bd)

### Thumbnails
- Generated from the first page of each archive
- Output format configurable: WebP (default), JPEG, PNG
- Configurable dimensions (default 300×400)
- Lazy generation: created on first access if missing
- Bulk operations: rebuild missing or regenerate all

### Book Renaming
- Rename all books in a series using a template pattern
- Four separate templates: regular, hors-série, intégrale, one-shot
- Default templates:
  - Regular: `{series_name} - T{volume_padded} - {title}`
  - HS: `{series_name} - HS {volume_padded}` (space before volume so HS without number gives "Series - HS")
  - Integral: `{series_name} - INT {volume_padded}`
  - One-shot: `{series_name}`
- Available variables: `{series_name}`, `{volume}`, `{volume_padded}`, `{title}`, `{authors}`, `{publish_date}`, `{isbn}`
- Volume padding auto-adjusts to match the widest volume in the series (after per-book overrides)
- Preview mode (dry-run) before execution, editable per book:
  - **Type**: requalify a book (`regular`/`hs`/`integral`/`oneshot`) and re-render with the matching template
  - **Volume**: correct a wrong volume number (falls back to the stored value; disabled for one-shots)
- Per-book overrides are applied to the preview, the on-disk rename and the DB
- Rename preserves `books.title`; only `books.volume` / `books.volume_type` are synced (no destructive title overwrite)
- Deduplication: suffixed filenames when template produces duplicates
- Null-variable cleanup: segments containing undefined variables are removed cleanly
- Templates saved globally in `app_settings` (keys: `rename_format`, `rename_format_hs`, `rename_format_int`, `rename_format_oneshot`)

### CBR to CBZ Conversion
- Convert RAR archives to ZIP format
- Tracked as background job with progress

---

## Series

### Automatic Aggregation
- Series derived from directory structure during scanning
- Books without series grouped as "unclassified"

### Series Metadata
- Description, publisher, start year, status (`ongoing`, `ended`, `completed`, `on_hold`, `hiatus`)
- Total volume count (from external providers or manual override)
- Authors (aggregated from books or metadata)
- **Field locking**: individual fields can be locked to prevent metadata sync from overwriting manual edits

### Missing Books Count
- Calculated as `max(total_volumes - regular_book_count, 0)` — only `volume_type = 'regular'` books counted
- HS, integral, and oneshot books are excluded from missing count calculations
- Based on `series.total_volumes` (user-editable), not external provider book count
- When `total_volumes` is NULL → 0 missing
- Manual edit of total_volumes immediately updates the missing count
- Series detail page uses `total_local` (regular only) for "X/Y – Z manquants" display

### Missing Volumes Display
- Toggle button in series detail to show/hide missing volumes (shown by default)
- Missing volumes displayed as grayed-out cards with provider cover images (grayscale)
- Data from `external_book_metadata` (cover_url, title, volume_number)
- Works for discovery series with no local books
- Merged with owned books and sorted by volume number

### Merge Series
- Merge a source series into a target series (absorb duplicates)
- Moves books, metadata links, available downloads, anilist links from source to target
- Keeps target's metadata links when both have the same provider
- Deletes the source series after merge
- Search modal on series detail page (cross-library search, API validates same library)

### Orphan Series Cleanup
- After stale book deletions during scan, series that lost ALL their books are removed (even with metadata links)
- This handles directory rename/deletion: the scanner tracks affected series_ids during stale file removal
- Discovery-created series (never had stale books deleted) are preserved by a separate cleanup pass
- Fallback cleanup: series with no books AND no metadata links AND no available downloads are also removed

### Series Rename Deduplication
- `get_or_create_series` checks both `name` and `original_name` to prevent duplicates
- Works in both API and indexer: after a user rename, lookups by old folder name find the renamed series
- Case-insensitive and accent-insensitive matching on both fields
- Scanner applies rename mapping in all code paths (including skipped directories)

### Filtering & Discovery
- Filter by: series name (partial match), reading status, series status, metadata provider linkage
- Sort by: name, reading status, book count
- **Missing books detection**: based on total_volumes - book_count

---

## Reading Progress

### Per-Book Tracking
- Three states: `unread` (default), `reading`, `read`
- Current page tracking when status is `reading`
- `last_read_at` timestamp auto-updated

### Series-Level Status
- Calculated from book statuses:
  - All read → series `read`
  - None read → series `unread`
  - Mixed → series `reading`

### Bulk Operations
- Mark entire series as read (updates all books)

---

## Search & Discovery

### Full-Text Search
- PostgreSQL-based (`ILIKE` + `pg_trgm`)
- Searches across: book titles, series names, authors (scalar and array fields), series metadata authors
- Case-insensitive partial matching
- Library-scoped filtering

### Results
- Book hits: title, authors, series, volume, language, kind
- Series hits: name, book count, read count, first book (for linking)
- Processing time included in response

---

## Authors

- Unique author aggregation from books and series metadata
- Per-author book and series count
- Searchable by name (partial match)
- Sortable by name or book count

---

## External Metadata

### Supported Providers
| Provider | Focus | API |
|----------|-------|-----|
| Google Books | General books (default fallback) | REST |
| ComicVine | Comics | REST (API key required) |
| BedéThèque | Franco-Belgian comics | HTML scraping |
| AniList | Manga/anime | GraphQL |
| Open Library | General books | REST |
| SensCritique | BD, manga, comics | GraphQL (`apollo.senscritique.com`) |

#### SensCritique Provider
- **Search**: `searchAutocomplete` with franchise deduplication, then per-edition breakdown via `groupProducts`
- **Edition selection**: each edition within a franchise is returned as a separate candidate with accurate `total_volumes` (e.g., "Naruto" 72 vol vs "Naruto (Édition Hokage)" 35 vol)
- **Edition detection**: parses product titles to extract edition name (pattern: "Title - Edition, tome X")
- **external_id format**: `franchise:{id}:edition:{base64(name)}`, backward compatible with `franchise:{id}`
- **Volumes**: `groupProducts` filtered by edition, deduplicates by volume number (keeps earliest), extracts volume numbers from titles
- **Series status**: inferred from latest release date — if within 18 months → `ongoing`, otherwise → `ended`
- **Description**: uses synopsis of the lowest-numbered volume within the edition (typically tome 1)
- **Rate limiting**: retry with exponential backoff on HTTP 429 (1s, 2s, 4s, 3 retries), 300ms throttle between requests in refresh jobs
- **Batch vs detailed mode**: batch metadata uses `detailed=true` for edition-level matching; refresh uses stored `external_id` directly (no search)

### Provider Configuration
- Global default provider with library-level override
- Fallback provider if primary is unavailable

### Matching Workflow
1. **Search**: query a provider, get candidates with confidence scores
2. **Match**: link a series to an external result (status `pending`)
3. **Approve**: validate and sync metadata to series and books
4. **Reject**: discard a match

### Volume Type Awareness
- Metadata matching only considers `volume_type = 'regular'` books
- HS, integral, and oneshot books are excluded from:
  - Missing count calculations (CTE and `get_missing_books`)
  - Book-to-metadata matching (sync, batch, refresh, rematch)
  - Confidence boost local_count
  - AniList reading progress push
- HS volume=1 will NOT match external book volume=1 (rematch_unlinked_books)

### Confidence Scoring
- **Provider-level**: name similarity (normalized Jaccard/containment), volume count bonus
- **Caller-level boost**: local book count (regular only) vs candidate total_volumes
  - Exact match (book_count == total_volumes): +0.30
  - Close match (±2 volumes): +0.15
- Candidates re-sorted by confidence after boost
- Auto-match threshold: confidence == 1.0

### Batch Processing
- Auto-match all series in a library via `metadata_batch` job
- Series processed by `updated_at ASC` (least recently updated first)
- Edition-level matching for SensCritique (detailed mode)
- Result statuses: `auto_matched`, `no_results`, `too_many_results`, `low_confidence`, `already_linked`

### Re-match Metadata
- Re-link all series to a different provider without losing existing links
- Skips series already linked to the target provider
- Deletes old provider link only after successful new match
- Uses detailed edition-level search for SensCritique

### Metadata Sync — Shared Logic

All three sync paths (approve, batch auto-match, refresh) use the same factored code (`shared_sync.rs`):

#### Series-Level Sync
Fields synced from provider to `series` table:
| Field | Source | Update rule |
|-------|--------|-------------|
| `description` | `metadata_json.description` (all providers store it there) | `COALESCE(NULLIF(new, ''), existing)` — only overwrites if new is non-empty |
| `authors` | `metadata_json.authors` or `candidate.authors` | Only if new array is non-empty |
| `publishers` | `metadata_json.publishers` or `candidate.publishers` | Only if new array is non-empty |
| `start_year` | `metadata_json.start_year` or `candidate.start_year` | `COALESCE(new, existing)` |
| `total_volumes` | `total_volumes_external` or `candidate.total_volumes` | `COALESCE(new, existing)` |
| `status` | `metadata_json.status` (normalized via `status_mappings` table) | `COALESCE(new, existing)` |
| `genres` | `metadata_json.genres` | Only if new array is non-empty |
| `cover_url` | `metadata_json.cover_url` or `candidate.cover_url` | `COALESCE(NULLIF(new, ''), existing)` |

All fields respect **locked_fields** — if a field is locked, the sync skips it entirely.

Uses `INSERT ... ON CONFLICT (library_id, name) DO UPDATE` so series is created if missing (e.g., from discovery).

#### Book-Level Sync
Books matched by: (1) volume number, then (2) title containment (case-insensitive). Only `volume_type IN ('regular', 'integral')` books participate. Volume 0 (T0 = HS in providers) is excluded from matching.

Fields synced from provider to `books` table:
| Field | Update rule |
|-------|-------------|
| `summary` | `COALESCE(NULLIF(new, ''), existing)` |
| `isbn` | `COALESCE(NULLIF(new, ''), existing)` |
| `publish_date` | `COALESCE(NULLIF(new, ''), existing)` |
| `language` | `COALESCE(NULLIF(new, ''), existing)` |
| `authors` / `author` | Only if new array is non-empty |

All fields respect **locked_fields**.

External book metadata is also stored in `external_book_metadata` (cover_url, title, volume_number, etc.) for missing volume display.

#### Provider Description Sources
| Provider | Description source |
|----------|-------------------|
| SensCritique (batch) | Synopsis of lowest-numbered volume from search autocomplete (20 items) |
| SensCritique (detailed) | Synopsis of lowest-numbered volume from full `groupProducts` per edition |
| Google Books | First volume's description encountered during grouping |
| AniList | Series-level description from GraphQL |
| ComicVine | Volume description (HTML stripped) |
| BDTheque | Series and album pages/album-list endpoint |
| BDphile | Series and album pages, including edition EAN |
| Open Library | First volume's description encountered during grouping |

All providers persist `description` in `metadata_json` so it survives the DB round-trip (match → approve → sync).

### Metadata Refresh
- Update approved links with latest data from providers
- Re-searches provider, finds matching candidate, diffs and syncs
- Change tracking reports per series and per book (field-level diffs)
- Non-destructive: only updates when provider has new data
- Uses the same shared sync functions as approve and batch
- BDTheque and BDphile use ISBN/EAN only as an additive fallback for new links; historical provider matches are not displaced
- Bédéthèque (`bedetheque.com`) is retired: the site is now behind a Cloudflare challenge. The provider code has been removed, existing library settings are migrated to BDTheque, and any remaining `bedetheque` metadata links are force-unlinked by migration `0114`.

### Field Locking
- Individual series and book fields can be locked to prevent external sync from overwriting manual edits
- Locked fields: `description`, `authors`, `publishers`, `start_year`, `total_volumes`, `status` (series); `summary`, `isbn`, `publish_date`, `language`, `authors` (books)
- Locking persisted in `locked_fields` JSONB column (e.g., `{"description": true}`)
- Sync reports include skipped fields (locked) vs updated fields

### AniList Reading Status Sync

Integration with AniList to synchronize reading progress in both directions for linked series.

#### Configuration
- AniList user ID required for pull/push operations
- Configured per library in the reading status provider settings
- Auto-push schedule configurable per library: `manual`, `hourly`, `daily`, `weekly`

#### Reading Status Match (`reading_status_match`)
- Pull reading progress from AniList and update local book statuses
- Maps AniList list status: `PLANNING` → `unread`, `CURRENT` → `reading`, `COMPLETED` → `read`
- Detailed per-series report: matched, updated, skipped, errors
- Rate limit handling: waits 10s and retries once on HTTP 429, aborts on second 429

#### Reading Status Push (`reading_status_push`)
- Differential push: only syncs series that changed since last push, have new books, or have never been synced
- Maps local status to AniList: `unread` → `PLANNING`, `reading` → `CURRENT`, `read` → `COMPLETED`
- Never auto-completes a series on AniList based solely on owned books (requires all books read)
- Per-series result tracking: pushed, skipped, no_books, error
- Same 429 retry logic as `reading_status_match`
- Auto-push schedule runs every minute check via indexer scheduler

---

## Discovery

Browse and add series to your library from external sources.

### Sources
| Tab | Source | Sort | Period filter |
|-----|--------|------|---------------|
| Nouveautés BD | SensCritique `productsByRelease` | Popularity | Month / Year / All time |
| Nouveautés Manga | SensCritique `productsByRelease` | Popularity | Month / Year / All time |
| Meilleures BD | SensCritique `productsByRelease` | Rating | Month / Year / All time |
| Meilleurs Manga | SensCritique `productsByRelease` | Rating | Month / Year / All time |
| SensCritique Top BD | SensCritique `top` (TOP_100_OUT_OF_TOP_10) | Rank | — |
| SensCritique Top Manga | SensCritique `poll` (id: 192836) | Rank | — |
| AniList | AniList trending manga | Popularity | — |
| Prowlarr | Prowlarr aggregated releases | Seeders | — |

### Caching
- **Top/poll lists**: cache infini (invalidation manuelle via bouton refresh)
- **Trending/best**: cache 24h
- **Prowlarr**: cache 7 jours
- Already-owned series filtered out server-side
- Hidden suggestions filtered out (per-user hide/unhide)

### Hide Suggestions
- Hide unwanted series from discovery results via `discovery_hidden` table
- Hidden panel to view and unhide previously hidden series
- Hidden by provider + external_id, persisted across sessions

### Add to Library
- Crée la série + metadata (description, auteurs, genres, statut, cover)
- Crée un metadata link pour `senscritique` (providers `sc_*` normalisés vers `senscritique`)
- Pas de metadata link pour `anilist` et `prowlarr`
- Discovery results grouped by franchise (series-level, not individual tomes)
- Revalidation des pages `/series` et `/libraries` après ajout

### Create Series (from UI)
- `POST /series/create` — create series with optional metadata linking in one request
- Library selector + name input + auto-search metadata provider
- Creates physical directory on disk
- Syncs series + book metadata if provider match selected
- Uses shared `create_series_with_metadata` helper (same as Discovery)

---

## External Integrations

### Komga Sync
- Import reading progress from a Komga server
- Matches local series/books by name
- Detailed sync report: matched, already read, newly marked, unmatched

### Prowlarr (Indexer Search)
- Search Prowlarr for missing volumes in a series
- Volume pattern matching against release titles (supports `T##`, `Tome ##`, `Vol ##`, ranges `1-10`, intégrales)
- Results: title, size, seeders/leechers, download URL, matched missing volumes
- **Download detection job**: auto-scan all series with missing volumes, report available releases in `available_downloads`
- Volume 0 (T0) excluded from missing volumes search
- **`prowlarr_no_results`** event (error level): Prowlarr returned 0 raw results (indexer problem)
- **`downloads_not_found`** event (info level): Prowlarr returned results but none matched missing volumes
- **Failed download indicator**: badge on available releases that had previous download errors (via `torrent_downloads` lateral join)
- **Release blacklist**: permanently hide unwanted releases so they don't reappear after next detection. Blacklist panel with unhide. Blacklisted titles filtered during detection.
- **Per-release `detected_at`**: each release stores when it was first discovered. Preserved across refreshes via `merge_releases` SQL function (same title = same release, keeps original date).
- **Downloads page sorting**: default sort by newest `detected_at` (Recent), also Seeders, Missing, Name. Relative date shown per series.

### qBittorrent
- Add torrents directly from Prowlarr search results or available downloads
- **Replace mode**: import all volumes from a torrent (bypass expected_volumes filter)
- **Duplicate torrent detection**: when the same torrent already exists in qBittorrent, detects it by magnet hash, reads real content_path, and launches import immediately if completed
- Connection test endpoint

### Torrent Import Pipeline
1. qBittorrent poller detects completed torrents (5-minute timeout for unresolved hashes)
2. Volume extraction from filenames (supports `Tome_01` underscore separator)
3. Series matching via `LOWER(unaccent())` (case + accent insensitive)
4. File naming from existing book reference (preserves naming convention)
5. Deduplication by format (cbz > cbr > pdf > epub)
6. **One-shot books**: files without volume number imported as-is (not skipped)
7. **Already-existing files**: counted as imported (not error)
8. **Same-volume re-download**: for single-volume **regular** books, the most recent file wins — a newer download replaces the older library file (deleted from disk), while an older download is skipped. HS/one-shots/integrals are excluded (they do not share tome numbering). Reading progress is restored by volume after replacement, even when the filename/extension changes (`Amulet - T2.cbr` → `Amulet - 02.cbz`), and the matched archive record is consumed so a later progress reset is not re-populated.
9. Detailed import result: imported files, skipped files with reasons, total source count
10. Import status: `imported` (any file imported), `no_files_imported` (none imported)
11. **Retry**: re-launch stuck imports via API (checks source files still exist)
12. Cleanup: remove torrent from qBittorrent, delete download directory
13. Post-import: scan job queued, metadata refresh if linked, `available_downloads` updated

---

## Notifications

### Telegram
- Real-time notifications via Telegram Bot API (`sendMessage` and `sendPhoto`)
- Configuration: bot token, chat ID, enable/disable toggle
- Test connection button in settings

### Granular Event Toggles
16 individually configurable notification events grouped by category:

| Category | Events |
|----------|--------|
| Scans | `scan_completed`, `scan_failed`, `scan_cancelled` |
| Thumbnails | `thumbnail_completed`, `thumbnail_failed`, `thumbnail_cancelled` |
| Conversion | `conversion_completed`, `conversion_failed`, `conversion_cancelled` |
| Metadata | `metadata_approved`, `metadata_batch_completed`, `metadata_refresh_completed` |
| Reading status | `reading_status_match_completed`, `reading_status_match_failed`, `reading_status_push_completed`, `reading_status_push_failed` |

### Thumbnail Images in Notifications
- Book cover thumbnails attached to applicable notifications (conversion, metadata approval)
- Uses `sendPhoto` multipart upload with fallback to text-only `sendMessage`

### Implementation
- Shared `crates/notifications` crate used by both API and indexer
- Fire-and-forget: notification failures are logged but never block the main operation
- Messages formatted in HTML with event-specific icons

---

## Page Rendering & Caching

### Page Extraction
- Render any page from supported archive formats
- 1-indexed page numbers

### Image Processing
- Output formats: original, JPEG, PNG, WebP
- Quality parameter (1–100)
- Max width parameter (1–2160 px)
- Configurable resampling filter: lanczos3, nearest, triangle/bilinear
- Concurrent render limit (default 8) with semaphore

### Caching
- **LRU in-memory cache**: byte-size limit configurable from the backoffice (128 MB by default), applied immediately
- **Disk cache**: SHA256-keyed, two-level directory structure
- Cache key = hash(path + page + format + quality + width)
- Configurable cache directory and max size
- Manual disk and memory cache clear via settings
- Live memory usage, cached page count, and configured limit in the backoffice

---

## Background Jobs

### Job Types
| Type | Description |
|------|-------------|
| `rebuild` | Incremental scan |
| `full_rebuild` | Full filesystem rescan |
| `rescan` | Deep rescan for new formats |
| `thumbnail_rebuild` | Generate missing thumbnails |
| `thumbnail_regenerate` | Clear and regenerate all thumbnails |
| `cbr_to_cbz` | Convert RAR to ZIP |
| `metadata_batch` | Auto-match series to metadata |
| `metadata_batch_rematch` | Re-match series to a different provider |
| `metadata_refresh` | Update approved metadata links |
| `download_detection` | Scan Prowlarr for missing volumes |
| `reading_status_match` | Pull reading progress from AniList to local |
| `reading_status_push` | Differential push of reading statuses to AniList |

### Job Lifecycle
- Status flow: `pending` → `running` → `success` | `failed` | `cancelled`
- Intermediate statuses: `extracting_pages`, `generating_thumbnails`
- Real-time progress via **Server-Sent Events** (SSE)
- Per-file error tracking (non-fatal: job continues on errors)
- Cancellation support for pending/running jobs

### Progress Tracking
- Percentage (0–100), current file, processed/total counts
- Timing: started_at, finished_at, phase2_started_at
- Stats JSON blob with job-specific metrics

---

## Authentication & Security

### Token System
- **Bootstrap token**: admin token via `API_BOOTSTRAP_TOKEN` env var
- **API tokens**: create, list, revoke with scopes
- Token format: `stl_{prefix}_{secret}` with Argon2 hashing
- Expiration dates, last usage tracking, revocation

### Access Control
- **Two scopes**: `admin` (full access) and `read` (read-only)
- Route-level middleware enforcement
- Rate limiting: configurable sliding window (default 120 req/s)

---

## Backoffice (Web UI)

### Dashboard
- Statistics cards: books, series, authors, libraries, pages, total size
- Interactive charts (recharts): donut, area, stacked bar, horizontal bar
- Reading status breakdown, format distribution, library distribution
- Currently reading section with progress bars
- Recently read section with cover thumbnails
- Reading activity over time (area chart)
- Books added over time (area chart)
- Per-library stacked reading progress
- Top series by book count
- Metadata coverage and provider breakdown

### Pages
- **Libraries**: list, create, delete, configure monitoring and metadata provider
- **Books**: global list with filtering/sorting, detail view with metadata and page rendering
- **Series**: global list, per-library view, detail with metadata management
- **Authors**: list with book/series counts, detail with author's books
- **Jobs**: history, live progress via SSE, error details
- **Tokens**: create, list, revoke API tokens
- **Settings**: image processing, cache, thumbnails, external services (Prowlarr, qBittorrent), notifications (Telegram)

### Interactive Features
- Real-time search with suggestions
- Metadata search and matching modals
- Prowlarr search modal for missing volumes
- Folder browser/picker for library paths
- Book/series editing forms
- Quick reading status toggles
- CBR to CBZ conversion trigger

---

## API

### Documentation
- OpenAPI/Swagger UI available at `/swagger-ui`
- Health check (`/health`), readiness (`/ready`), Prometheus metrics (`/metrics`)

### Public Endpoints (no auth)
- `GET /health`, `GET /ready`, `GET /metrics`, `GET /swagger-ui`

### Read Endpoints (read scope)
- Libraries, books, series, authors listing and detail
- Book pages and thumbnails
- Reading progress get/update
- Full-text search, collection statistics
- Metadata links listing (`GET /metadata/links?series_id=...`)
- Missing books with cover URLs (`GET /metadata/missing/{link_id}`)

### Admin Endpoints (admin scope)
- Library CRUD and configuration
- Book metadata editing, CBR conversion
- Series metadata editing
- Indexing job management (trigger, cancel, stream)
- API token management
- Metadata operations (search, match, approve, reject, batch, refresh)
- External integrations (Prowlarr, qBittorrent, Komga)
- Application settings and cache management

### OpenAPI
- Two separate specs: **Client API** (read scope) and **Admin API** (all endpoints)
- Swagger UI at `/swagger-ui` (client) and `/admin/swagger-ui` (admin)
- Dropdown to switch between specs

---

## Database

### Key Design Decisions
- PostgreSQL with `pg_trgm` for full-text search (no external search engine)
- All deletions cascade from libraries
- Unique constraints: file paths, token prefixes, metadata links (library + series + provider)
- **Series name matching**: always `LOWER(unaccent(name))` — case and accent insensitive to prevent duplicates from different sources (discovery, torrent import, scanner). Also checks `original_name` for renamed series.
- **Unified job events**: `index_job_events` table replaces per-job-type result tables
- Directory mtime caching for incremental scan optimization
- Connection pool: 10 (API), 20 (indexer)

### Archive Resilience
- CBZ: fallback streaming reader if central directory corrupted
- CBR: RAR extraction via system `unar`, fallback to CBZ parsing
- PDF: `pdfinfo` for page count, `pdftoppm` for rendering
- EPUB: ZIP-based extraction
- FD exhaustion detection: aborts if too many consecutive IO errors
