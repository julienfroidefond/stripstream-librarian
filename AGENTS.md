# AGENTS.md - Agent Coding Guidelines for Stripstream Librarian

This file provides guidelines for agentic coding agents operating in this repository.

---

## 1. Build, Lint, and Test Commands

### Build Commands

```bash
# Build debug version (fastest for development)
cargo build

# Build release version (optimized)
cargo build --release

# Build specific crate
cargo build -p api
cargo build -p indexer

# Watch mode for development (requires cargo-watch)
cargo watch -x build
```

### Lint & Format Commands

```bash
# Run clippy lints
cargo clippy

# Fix auto-fixable clippy warnings
cargo clippy --fix

# Format code
cargo fmt

# Check formatting without making changes
cargo fmt -- --check
```

### Test Commands

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p api
cargo test -p indexer
cargo test -p parsers

# Run a single test by name
cargo test test_name_here

# Run tests with output display
cargo test -- --nocapture

# Run doc tests
cargo test --doc
```

### Database Migrations

```bash
# Run migrations manually (via sqlx CLI)
# Ensure DATABASE_URL is set, then:
sqlx migrate run

# Create new migration
sqlx migrate add -r migration_name
```

### Docker Development

`docker-compose.yml` est à la **racine** du projet (pas dans `infra/`).

```bash
# Start infrastructure only
docker compose up -d postgres meilisearch

# Start full stack
docker compose up -d

# View logs
docker compose logs -f api
docker compose logs -f indexer
```

---

## 2. Code Style Guidelines

### General Principles

- **Conciseness**: Keep responses short and direct. Avoid unnecessary preamble or explanation.
- **Idiomatic Rust**: Follow Rust best practices and ecosystem conventions.
- **Error Handling**: Use `anyhow::Result<T>` for application code, `std::io::Result<T>` for simple file operations.
- **Async**: Use `tokio` for async runtime. Prefer `#[tokio::main]` over manual runtime.

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

- **Absolute imports** for workspace crates: `use parsers::{detect_format, parse_metadata};`
- **Standard library** imports: `use std::path::Path;`
- **External crates**: `use sqlx::{postgres::PgPoolOptions, Row};`
- **Group by**: std → external → workspace → local (with blank lines between)

```rust
use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use crate::AppState;
```

### Error Handling

- Use `anyhow` for application-level error handling with context
- Use `with_context()` for adding context to errors
- Return `Result<T, ApiError>` in API handlers
- Use `?` operator instead of manual match/unwrap where possible

```rust
// Good
fn process_book(path: &Path) -> anyhow::Result<Book> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open file: {}", path.display()))?;
    // ...
}

// Good - API error handling
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

- Use **raw SQL queries** with `sqlx::query()` and `sqlx::query_scalar()`
- Prefer **batch operations** using `UNNEST` for bulk inserts/updates
- Always use **parameterized queries** (`$1`, `$2`, etc.) - never string interpolation
- Follow existing patterns for transactions:

```rust
let mut tx = pool.begin().await?;
// ... queries ...
tx.commit().await?;
```

### Async/Tokio

- Use `tokio::spawn` for background tasks
- Use `spawn_blocking` for CPU-bound work (image processing, file I/O)
- Keep async handlers non-blocking
- Use `tokio::time::timeout` for operations with timeouts

```rust
let bytes = tokio::time::timeout(
    Duration::from_secs(60),
    tokio::task::spawn_blocking(move || {
        render_page(&abs_path_clone, n)
    }),
)
.await
.map_err(|_| ApiError::internal("timeout"))?
.map_err(ApiError::internal)?;
```

### Structs and Serialization

- Use `#[derive(Serialize, Deserialize, ToSchema)]` for API types
- Add `utoipa` schemas for OpenAPI documentation
- Use `Option<T>` for nullable fields
- Document public structs briefly

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

### Performance Considerations

- Use **batch operations** for database inserts/updates (100 items recommended)
- Use **parallel iterators** (`rayon::par_iter()`) for CPU-intensive scans
- Implement **caching** for expensive operations (see `pages.rs` for disk/memory cache examples)
- Use **streaming** for large data where applicable

### Testing

- Currently there are no test files - consider adding unit tests for:
  - Parser functions
  - Thumbnail generation
  - Configuration parsing
- Use `#[cfg(test)]` modules for integration tests

---

## 3. Project Structure

```
stripstream-librarian/
├── apps/
│   ├── api/           # REST API (axum) — port 7080
│   │   └── src/       # books.rs, pages.rs, thumbnails.rs, state.rs, auth.rs...
│   ├── indexer/       # Background indexing service — port 7081
│   │   └── src/       # worker.rs, scanner.rs, batch.rs, scheduler.rs, watcher.rs...
│   └── backoffice/    # Next.js admin UI — port 7082
├── crates/
│   ├── core/          # Shared config (env vars)
│   │   └── src/config.rs
│   └── parsers/       # Book parsing (CBZ, CBR, PDF)
├── infra/
│   └── migrations/    # SQL migrations (sqlx)
├── data/
│   └── thumbnails/    # Thumbnails générés par l'API
├── libraries/         # Book storage (mounted volume)
└── docker-compose.yml # À la racine (pas dans infra/)
```

### Key Files

| File | Purpose |
|------|---------|
| `apps/api/src/books.rs` | Book CRUD endpoints |
| `apps/api/src/pages.rs` | Page rendering & caching (LRU + disk) |
| `apps/api/src/thumbnails.rs` | Thumbnail generation (triggered by indexer) |
| `apps/api/src/state.rs` | AppState, Semaphore concurrent_renders |
| `apps/indexer/src/scanner.rs` | Filesystem scan, rayon parallel parsing |
| `apps/indexer/src/batch.rs` | Bulk DB ops via UNNEST |
| `apps/indexer/src/worker.rs` | Job loop, watcher, scheduler orchestration |
| `crates/parsers/src/lib.rs` | Format detection, metadata parsing |
| `crates/core/src/config.rs` | Configuration from environment |
| `infra/migrations/*.sql` | Database schema |

---

## 4. Common Patterns

### Configuration from Environment

```rust
// In crates/core/src/config.rs
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

### Path Remapping

```rust
fn remap_libraries_path(path: &str) -> String {
    if let Ok(root) = std::env::var("LIBRARIES_ROOT_PATH") {
        if path.starts_with("/libraries/") {
            return path.replacen("/libraries", &root, 1);
        }
    }
    path.to_string()
}
```

---

## 5. Important Notes

- **Workspace**: This is a Cargo workspace. Always specify the package when building specific apps.
- **Dependencies**: External crates are defined in workspace `Cargo.toml`, not individual `Cargo.toml`.
- **Database**: PostgreSQL is required. Run migrations before starting services.
- **External Tools**: 4 system tools required — `unrar` (CBR page count), `unar` (CBR extraction), `pdfinfo` (PDF page count), `pdftoppm` (PDF page render). Note: `unrar` and `unar` are distinct tools.
- **Thumbnails**: generated by the **API** service (not the indexer). The indexer triggers a checkup via `POST /index/jobs/:id/thumbnails/checkup` after indexing.
- **Sub-AGENTS.md**: module-specific guidelines in `apps/api/`, `apps/indexer/`, `apps/backoffice/`, `crates/parsers/`.
