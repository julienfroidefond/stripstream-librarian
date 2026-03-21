# Stripstream Librarian

A comprehensive comic book and e-book management system with automatic indexing, full-text search, and a modern web interface.

## Architecture

The project consists of the following components:

- **API** (`apps/api/`) - Rust-based REST API service
- **Indexer** (`apps/indexer/`) - Rust-based background indexing service
- **Backoffice** (`apps/backoffice/`) - Next.js web administration interface
- **Infrastructure** (`infra/`) - Docker Compose setup with PostgreSQL

## Quick Start

### Prerequisites

- Docker and Docker Compose
- (Optional) Rust toolchain for local development
- (Optional) Node.js 18+ for backoffice development

### Environment Setup

1. Copy the example environment file:
   ```bash
   cp .env.example .env
   ```

2. Edit `.env` and set secure values for:
   - `API_BOOTSTRAP_TOKEN` - Bootstrap token for initial API authentication

### Running with Docker

```bash
docker compose up -d
```

This will start:
- PostgreSQL (port 6432)
- API service (port 7080)
- Indexer service (port 7081)
- Backoffice web UI (port 7082)

### Accessing the Application

- **Backoffice**: http://localhost:7082
- **API**: http://localhost:7080

### Default Credentials

The default bootstrap token is configured in your `.env` file. Use this for initial API authentication.

## Development

### Local Development (without Docker)

#### API & Indexer (Rust)

```bash
# Start dependencies
docker compose up -d postgres

# Run API
cd apps/api
cargo run

# Run Indexer (in another terminal)
cd apps/indexer
cargo run
```

#### Backoffice (Next.js)

```bash
cd apps/backoffice
npm install
npm run dev
```

The backoffice will be available at http://localhost:7082

## Features

> For the full feature list, business rules, and API details, see [docs/FEATURES.md](docs/FEATURES.md).

### Libraries
- Multi-library management with per-library configuration
- Incremental and full scanning, real-time filesystem watcher
- Per-library metadata provider selection (Google Books, ComicVine, BedéThèque, AniList, Open Library)

### Books & Series
- **Formats**: CBZ, CBR, PDF, EPUB
- Automatic metadata extraction (title, series, volume, authors, page count) from filenames and directory structure
- Series aggregation with missing volume detection
- Thumbnail generation (WebP/JPEG/PNG) with lazy generation and bulk rebuild
- CBR → CBZ conversion

### Reading Progress
- Per-book tracking: unread / reading / read with current page
- Series-level aggregated reading status
- Bulk mark-as-read for series

### Search & Discovery
- Full-text search across titles, authors, and series (PostgreSQL `pg_trgm`)
- Author listing with book/series counts
- Filtering by reading status, series status, format, metadata provider

### External Metadata
- Search, match, approve/reject workflow with confidence scoring
- Batch auto-matching and scheduled metadata refresh
- Field locking to protect manual edits from sync

### External Integrations
- **Komga**: import reading progress
- **Prowlarr**: search for missing volumes
- **qBittorrent**: add torrents directly from search results

### Background Jobs
- Rebuild, rescan, thumbnail generation, metadata batch, CBR conversion
- Real-time progress via Server-Sent Events (SSE)
- Job history, error tracking, cancellation

### Page Rendering
- On-demand page extraction from all formats
- Image processing (format, quality, max width, resampling filter)
- LRU in-memory + disk cache

### Security
- Token-based auth (`admin` / `read` scopes) with Argon2 hashing
- Rate limiting, token expiration and revocation

### Web UI (Backoffice)
- Dashboard with statistics, charts, and reading progress
- Library, book, series, author management
- Live job monitoring, metadata search modals, settings panel

## Environment Variables

Variables marquées **required** doivent être définies. Les autres ont une valeur par défaut.

### Partagées (API + Indexer)

| Variable | Description | Défaut |
|----------|-------------|--------|
| `DATABASE_URL` | **required** — Connexion PostgreSQL | — |

### API

| Variable | Description | Défaut |
|----------|-------------|--------|
| `API_BOOTSTRAP_TOKEN` | **required** — Token admin initial | — |
| `API_LISTEN_ADDR` | Adresse d'écoute | `0.0.0.0:7080` |

### Indexer

| Variable | Description | Défaut |
|----------|-------------|--------|
| `INDEXER_LISTEN_ADDR` | Adresse d'écoute | `0.0.0.0:7081` |
| `INDEXER_SCAN_INTERVAL_SECONDS` | Intervalle de scan du watcher | `5` |
| `THUMBNAIL_ENABLED` | Activer la génération de thumbnails | `true` |
| `THUMBNAIL_DIRECTORY` | Dossier de stockage des thumbnails | `/data/thumbnails` |
| `THUMBNAIL_WIDTH` | Largeur max des thumbnails (px) | `300` |
| `THUMBNAIL_HEIGHT` | Hauteur max des thumbnails (px) | `400` |
| `THUMBNAIL_QUALITY` | Qualité WebP (0–100) | `80` |
| `THUMBNAIL_FORMAT` | Format de sortie | `webp` |

### Backoffice

| Variable | Description | Défaut |
|----------|-------------|--------|
| `API_BOOTSTRAP_TOKEN` | **required** — Token d'accès à l'API | — |
| `API_BASE_URL` | URL interne de l'API (dans le réseau Docker) | `http://api:7080` |

## API Documentation

The API is documented with OpenAPI/Swagger. When running locally, access the docs at:

```
http://localhost:7080/swagger-ui
```

## Project Structure

```
stripstream-librarian/
├── apps/
│   ├── api/              # Rust REST API
│   ├── indexer/          # Rust background indexer
│   └── backoffice/       # Next.js web UI
├── infra/
│   └── migrations/       # SQL database migrations
├── libraries/            # Book storage (mounted volume)
└── .env                  # Environment configuration
```

## Docker Registry

Images are built and pushed to Docker Hub with the naming convention `docker.io/{owner}/stripstream-{service}`.

### Publishing Images (Maintainers)

To build and push all service images to the registry:

```bash
# Login to Docker Hub first
docker login -u julienfroidefond32

# Build and push all images
./scripts/docker-push.sh
```

This script will:
- Build images for `api`, `indexer`, and `backoffice`
- Tag them with the current version (from `Cargo.toml`) and `latest`
- Push to the registry

### Using Published Images

To use the pre-built images in your own `docker-compose.yml`:

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: stripstream
      POSTGRES_USER: stripstream
      POSTGRES_PASSWORD: stripstream
    volumes:
      - postgres_data:/var/lib/postgresql/data

  api:
    image: julienfroidefond32/stripstream-api:latest
    ports:
      - "7080:7080"
    volumes:
      - ./libraries:/libraries
      - ./data/thumbnails:/data/thumbnails
    environment:
      # --- Required ---
      DATABASE_URL: postgres://stripstream:stripstream@postgres:5432/stripstream
      API_BOOTSTRAP_TOKEN: your_bootstrap_token  # required — change this
      # --- Optional (defaults shown) ---
      # API_LISTEN_ADDR: 0.0.0.0:7080

  indexer:
    image: julienfroidefond32/stripstream-indexer:latest
    ports:
      - "7081:7081"
    volumes:
      - ./libraries:/libraries
      - ./data/thumbnails:/data/thumbnails
    environment:
      # --- Required ---
      DATABASE_URL: postgres://stripstream:stripstream@postgres:5432/stripstream
      # --- Optional (defaults shown) ---
      # INDEXER_LISTEN_ADDR: 0.0.0.0:7081
      # INDEXER_SCAN_INTERVAL_SECONDS: 5
      # THUMBNAIL_ENABLED: true
      # THUMBNAIL_DIRECTORY: /data/thumbnails
      # THUMBNAIL_WIDTH: 300
      # THUMBNAIL_HEIGHT: 400
      # THUMBNAIL_QUALITY: 80
      # THUMBNAIL_FORMAT: webp

  backoffice:
    image: julienfroidefond32/stripstream-backoffice:latest
    ports:
      - "7082:7082"
    environment:
      # --- Required ---
      API_BOOTSTRAP_TOKEN: your_bootstrap_token  # must match api above
      # --- Optional (defaults shown) ---
      # API_BASE_URL: http://api:7080

volumes:
  postgres_data:
```

## License

[Your License Here]
