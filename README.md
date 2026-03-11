# Stripstream Librarian

A comprehensive comic book and e-book management system with automatic indexing, full-text search, and a modern web interface.

## Architecture

The project consists of the following components:

- **API** (`apps/api/`) - Rust-based REST API service
- **Indexer** (`apps/indexer/`) - Rust-based background indexing service
- **Backoffice** (`apps/backoffice/`) - Next.js web administration interface
- **Infrastructure** (`infra/`) - Docker Compose setup with PostgreSQL and Meilisearch

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
   - `MEILI_MASTER_KEY` - Master key for Meilisearch
   - `API_BOOTSTRAP_TOKEN` - Bootstrap token for initial API authentication

### Running with Docker

```bash
cd infra
docker compose up -d
```

This will start:
- PostgreSQL (port 6432)
- Meilisearch (port 7700)
- API service (port 7080)
- Indexer service (port 7081)
- Backoffice web UI (port 7082)

### Accessing the Application

- **Backoffice**: http://localhost:7082
- **API**: http://localhost:7080
- **Meilisearch**: http://localhost:7700

### Default Credentials

The default bootstrap token is configured in your `.env` file. Use this for initial API authentication.

## Development

### Local Development (without Docker)

#### API & Indexer (Rust)

```bash
# Start dependencies
cd infra
docker compose up -d postgres meilisearch

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

The backoffice will be available at http://localhost:3000

## Features

### Libraries Management
- Create and manage multiple libraries
- Configure automatic scanning schedules (hourly, daily, weekly)
- Real-time file watcher for instant indexing
- Full and incremental rebuild options

### Books Management
- Support for CBZ, CBR, and PDF formats
- Automatic metadata extraction
- Series and volume detection
- Full-text search with Meilisearch

### Jobs Monitoring
- Real-time job progress tracking
- Detailed statistics (scanned, indexed, removed, errors)
- Job history and logs
- Cancel pending jobs

### Search
- Full-text search across titles, authors, and series
- Library filtering
- Real-time suggestions

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `API_LISTEN_ADDR` | API service bind address | `0.0.0.0:7080` |
| `INDEXER_LISTEN_ADDR` | Indexer service bind address | `0.0.0.0:7081` |
| `BACKOFFICE_PORT` | Backoffice web UI port | `7082` |
| `DATABASE_URL` | PostgreSQL connection string | `postgres://stripstream:stripstream@postgres:5432/stripstream` |
| `MEILI_URL` | Meilisearch connection URL | `http://meilisearch:7700` |
| `MEILI_MASTER_KEY` | Meilisearch master key (required) | - |
| `API_BOOTSTRAP_TOKEN` | Initial API admin token (required) | - |
| `INDEXER_SCAN_INTERVAL_SECONDS` | Watcher scan interval | `5` |
| `LIBRARIES_ROOT_PATH` | Path to libraries directory | `/libraries` |

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
│   ├── docker-compose.yml
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

  meilisearch:
    image: getmeili/meilisearch:v1.12
    environment:
      MEILI_MASTER_KEY: ${MEILI_MASTER_KEY}

  api:
    image: julienfroidefond32/stripstream-api:latest
    env_file:
      - .env
    ports:
      - "7080:7080"
    volumes:
      - ${LIBRARIES_HOST_PATH:-./libraries}:/libraries
      - ${THUMBNAILS_HOST_PATH:-./data/thumbnails}:/data/thumbnails

  indexer:
    image: julienfroidefond32/stripstream-indexer:latest
    env_file:
      - .env
    ports:
      - "7081:7081"
    volumes:
      - ${LIBRARIES_HOST_PATH:-./libraries}:/libraries
      - ${THUMBNAILS_HOST_PATH:-./data/thumbnails}:/data/thumbnails

  backoffice:
    image: julienfroidefond32/stripstream-backoffice:latest
    env_file:
      - .env
    environment:
      - PORT=7082
      - HOST=0.0.0.0
    ports:
      - "7082:7082"

volumes:
  postgres_data:
```

## License

[Your License Here]
