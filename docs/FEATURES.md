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
- **Series**: derived from directory structure (first directory level under library root)
- **Volume**: extracted from filename with pattern detection:
  - `T##` (Tome) — most common for French comics
  - `Vol.##`, `Vol ##`, `Volume ##`
  - `###` (standalone number)
  - `-## ` (dash-separated)
- **Author(s)**: single scalar and array support
- **Page count**: extracted from archive analysis
- **Language**, **kind** (ebook, comic, bd)

### Thumbnails
- Generated from the first page of each archive
- Output format configurable: WebP (default), JPEG, PNG
- Configurable dimensions (default 300×400)
- Lazy generation: created on first access if missing
- Bulk operations: rebuild missing or regenerate all

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
- Total volume count (from external providers)
- Authors (aggregated from books or metadata)

### Filtering & Discovery
- Filter by: series name (partial match), reading status, series status, metadata provider linkage
- Sort by: name, reading status, book count
- **Missing books detection**: identifies gaps in volume numbering within a series

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
| Provider | Focus |
|----------|-------|
| Google Books | General books (default fallback) |
| ComicVine | Comics |
| BedéThèque | Franco-Belgian comics |
| AniList | Manga/anime |
| Open Library | General books |

### Provider Configuration
- Global default provider with library-level override
- Fallback provider if primary is unavailable

### Matching Workflow
1. **Search**: query a provider, get candidates with confidence scores
2. **Match**: link a series to an external result (status `pending`)
3. **Approve**: validate and sync metadata to series and books
4. **Reject**: discard a match

### Batch Processing
- Auto-match all series in a library via `metadata_batch` job
- Configurable confidence threshold
- Result statuses: `auto_matched`, `no_results`, `too_many_results`, `low_confidence`, `already_linked`

### Metadata Refresh
- Update approved links with latest data from providers
- Change tracking reports per series/book
- Non-destructive: only updates when provider has new data

### Field Locking
- Individual book fields can be locked to prevent external sync from overwriting manual edits

---

## External Integrations

### Komga Sync
- Import reading progress from a Komga server
- Matches local series/books by name
- Detailed sync report: matched, already read, newly marked, unmatched

### Prowlarr (Indexer Search)
- Search Prowlarr for missing volumes in a series
- Volume pattern matching against release titles
- Results: title, size, seeders/leechers, download URL, matched missing volumes

### qBittorrent
- Add torrents directly from Prowlarr search results
- Connection test endpoint

---

## Notifications

### Telegram
- Real-time notifications via Telegram Bot API (`sendMessage` and `sendPhoto`)
- Configuration: bot token, chat ID, enable/disable toggle
- Test connection button in settings

### Granular Event Toggles
12 individually configurable notification events grouped by category:

| Category | Events |
|----------|--------|
| Scans | `scan_completed`, `scan_failed`, `scan_cancelled` |
| Thumbnails | `thumbnail_completed`, `thumbnail_failed`, `thumbnail_cancelled` |
| Conversion | `conversion_completed`, `conversion_failed`, `conversion_cancelled` |
| Metadata | `metadata_approved`, `metadata_batch_completed`, `metadata_refresh_completed` |

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
- **LRU in-memory cache**: 512 entries
- **Disk cache**: SHA256-keyed, two-level directory structure
- Cache key = hash(path + page + format + quality + width)
- Configurable cache directory and max size
- Manual cache clear via settings

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
| `metadata_refresh` | Update approved metadata links |

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

### Admin Endpoints (admin scope)
- Library CRUD and configuration
- Book metadata editing, CBR conversion
- Series metadata editing
- Indexing job management (trigger, cancel, stream)
- API token management
- Metadata operations (search, match, approve, reject, batch, refresh)
- External integrations (Prowlarr, qBittorrent, Komga)
- Application settings and cache management

---

## Database

### Key Design Decisions
- PostgreSQL with `pg_trgm` for full-text search (no external search engine)
- All deletions cascade from libraries
- Unique constraints: file paths, token prefixes, metadata links (library + series + provider)
- Directory mtime caching for incremental scan optimization
- Connection pool: 10 (API), 20 (indexer)

### Archive Resilience
- CBZ: fallback streaming reader if central directory corrupted
- CBR: RAR extraction via system `unar`, fallback to CBZ parsing
- PDF: `pdfinfo` for page count, `pdftoppm` for rendering
- EPUB: ZIP-based extraction
- FD exhaustion detection: aborts if too many consecutive IO errors
