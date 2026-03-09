# Stripstream Librarian

Gestionnaire de bibliothèque de bandes dessinées/ebooks. Workspace Cargo multi-crates avec backoffice Next.js.

## Architecture

| Service | Dossier | Port local |
|---------|---------|------------|
| API REST (axum) | `apps/api/` | 7080 |
| Indexer (background) | `apps/indexer/` | 7081 |
| Backoffice (Next.js) | `apps/backoffice/` | 7082 |
| PostgreSQL | infra | 6432 |
| Meilisearch | infra | 7700 |

Crates partagés : `crates/core` (config env), `crates/parsers` (CBZ/CBR/PDF).

## Commandes

```bash
# Build
cargo build                     # workspace entier
cargo build -p api              # crate spécifique
cargo build --release           # version optimisée

# Linting / format
cargo clippy
cargo fmt

# Tests
cargo test
cargo test -p parsers

# Infra (dépendances uniquement) — docker-compose.yml est à la racine
docker compose up -d postgres meilisearch

# Backoffice dev
cd apps/backoffice && npm install && npm run dev  # http://localhost:7082

# Migrations
sqlx migrate run  # DATABASE_URL doit être défini
```

## Environnement

```bash
cp .env.example .env  # puis éditer les valeurs REQUIRED
```

Variables **requises** au démarrage : `DATABASE_URL`, `MEILI_URL`, `MEILI_MASTER_KEY`, `API_BOOTSTRAP_TOKEN`.

## Gotchas

- **Dépendances système** : 4 outils requis — `unrar` (CBR listing), `unar` (CBR extraction), `pdfinfo` (PDF page count), `pdftoppm` (PDF rendu). `unrar` ≠ `unar`.
- **Port backoffice** : `npm run dev` écoute sur **7082**, pas 3000.
- **LIBRARIES_ROOT_PATH** : les chemins en DB commencent par `/libraries/` ; en dev local, définir cette variable pour remapper vers le dossier réel.
- **Thumbnails** : stockés dans `THUMBNAIL_DIRECTORY` (défaut `/data/thumbnails`), générés par **l'API** (pas l'indexer) — l'indexer déclenche un checkup via `POST /index/jobs/:id/thumbnails/checkup`.
- **Workspace Cargo** : les dépendances externes sont définies dans le `Cargo.toml` racine, pas dans les crates individuels.
- **Migrations** : dossier `infra/migrations/`, géré par sqlx. Toujours migrer avant de démarrer les services.

## Fichiers clés

| Fichier | Rôle |
|---------|------|
| `crates/core/src/config.rs` | Config depuis env (API, Indexer, AdminUI) |
| `crates/parsers/src/lib.rs` | Détection format, extraction métadonnées |
| `apps/api/src/books.rs` | Endpoints CRUD livres |
| `apps/api/src/pages.rs` | Rendu pages + cache LRU |
| `apps/indexer/src/scanner.rs` | Scan filesystem |
| `infra/migrations/*.sql` | Schéma DB |

> Voir `AGENTS.md` pour les conventions de code détaillées (error handling, patterns sqlx, async/tokio).
> Des `AGENTS.md` spécifiques existent dans `apps/api/`, `apps/indexer/`, `apps/backoffice/`, `crates/parsers/`.
