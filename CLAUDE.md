# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Stripstream Librarian — gestionnaire de bibliothèque de bandes dessinées/ebooks (CBZ, CBR, PDF, EPUB). Workspace Cargo multi-crates avec backoffice Next.js.

## Architecture

| Service | Dossier | Port local | Framework |
|---------|---------|------------|-----------|
| API REST | `apps/api/` | 7080 | axum |
| Indexer (background) | `apps/indexer/` | 7081 | axum (minimal) |
| Backoffice | `apps/backoffice/` | 7082 | Next.js 16 / React 19 |
| PostgreSQL | infra | 6432 | — |

Crates partagés : `crates/core` (config env), `crates/parsers` (CBZ/CBR/PDF), `crates/notifications` (Telegram).

### Indexer 2-Phase Pipeline

1. **Discovery** (`scanner.rs`) — WalkDir + metadata from filename only (zero archive I/O). Inserts books with `page_count = NULL` so they're visible immediately. Skips unchanged directories via `directory_mtimes` table.
2. **Analysis** (`analyzer.rs`) — Opens archives, extracts page count + first page, generates WebP thumbnails. Processes books where `page_count IS NULL`.

Fingerprint = SHA256(size + mtime + filename) — detects changes without re-reading files.

## Commands

```bash
# Build
cargo build                     # workspace entier
cargo build -p api              # crate spécifique
cargo build --release           # version optimisée

# Linting / format
cargo clippy
cargo fmt
cargo fmt -- --check            # vérification sans modification

# Tests
cargo test                      # tous les tests
cargo test -p parsers           # tests d'un crate
cargo test test_name            # test unique par nom
cargo test -- --nocapture       # avec affichage stdout

# Infra — docker-compose.yml est à la racine
docker compose up -d postgres

# Backoffice dev
cd apps/backoffice && npm install && npm run dev  # http://localhost:7082

# Migrations
sqlx migrate run                # DATABASE_URL doit être défini
sqlx migrate add -r nom_migration  # créer une migration (up + down)
```

## Environment

```bash
cp .env.example .env  # puis éditer les valeurs REQUIRED
```

Variables **requises** : `DATABASE_URL`, `API_BOOTSTRAP_TOKEN`, `ADMIN_USERNAME`, `ADMIN_PASSWORD`, `SESSION_SECRET` (min 32 chars).

### Logging (RUST_LOG)

Domaines : `indexer`, `scan`, `extraction`, `thumbnail`, `watcher`. Niveaux : error, warn, info, debug, trace.
Défaut : `indexer=info,scan=info,extraction=info,thumbnail=warn,watcher=info`.

## Code Conventions

### Error Handling
- `anyhow::Result<T>` + `with_context()` pour le code applicatif
- `Result<T, ApiError>` dans les handlers API — constructeurs : `ApiError::bad_request()`, `not_found()`, `internal()`, `unauthorized()`, `forbidden()`
- `?` operator systématiquement, jamais de `unwrap()` en production

### Database (sqlx)
- Raw SQL avec `sqlx::query()` — pas d'ORM
- Paramètres positionnels (`$1`, `$2`) — jamais d'interpolation de chaînes
- **Batch via UNNEST** pour les opérations massives (voir `batch.rs`)
- Transactions : `let mut tx = pool.begin().await?; ... tx.commit().await?;`

### Async/Tokio
- `spawn_blocking` obligatoire pour CPU-bound (rendu pages, ouverture archives, génération thumbnails)
- `tokio::spawn` pour les tâches background
- Concurrence bornée via `Semaphore` (pages) ou `for_each_concurrent` (analysis)

### API Types
- `#[derive(Serialize, Deserialize, ToSchema)]` pour les types exposés
- `utoipa` pour la doc OpenAPI — spec sur `/openapi.json`, UI sur `/swagger-ui`

### Factorisation
- **Toujours factoriser** : extraire la logique dupliquée dans des fonctions/modules réutilisables.
- **Splitter les gros fichiers** : quand un fichier devient trop long, le découper en modules plus petits et cohérents. Ne pas hésiter à refactorer proactivement les fichiers existants qui sont trop volumineux.

### Imports (ordre)
std → external crates → workspace crates → local (`crate::`)

### Backoffice (Next.js)
- **Privilégier le server-side** : maximiser l'utilisation des Server Components pour les performances et la réutilisation. Ne passer en `"use client"` que pour l'interactivité strictement nécessaire (event handlers, hooks React).
- App Router, Tailwind CSS v4, dark/light via `next-themes`
- Tous les appels API dans `lib/api.ts` (types DTO + fetch)
- Composants UI génériques dans `app/components/ui/` — les réutiliser plutôt que du HTML brut

## Tests

La couverture de tests est actuellement faible. Lors de tout ajout ou modification de code, **ajouter des tests unitaires** pour le code touché. Priorités :
- Fonctions utilitaires et logique pure (parsing, fingerprint, remapping, extraction volume)
- Handlers API (réponses attendues, cas d'erreur)
- Opérations batch et logique DB (via mocks ou tests d'intégration)

Utiliser `#[cfg(test)]` pour les modules de test intégrés. Pour les tests nécessitant une DB, documenter les prérequis dans le test.

## Gotchas

- **Dépendances système** : 4 outils requis — `unrar` (CBR listing), `unar` (CBR extraction), `pdfinfo` (PDF page count), `pdftoppm` (PDF rendu). `unrar` ≠ `unar`. Tous issus de paquets différents.
- **Port backoffice** : `npm run dev` écoute sur **7082**, pas 3000 (défini dans `package.json`).
- **LIBRARIES_ROOT_PATH** : les chemins en DB commencent par `/libraries/`. En dev local, définir cette variable pour remapper vers le dossier réel. Utiliser `remap_libraries_path()` / `unmap_libraries_path()` (dans `utils.rs`).
- **Thumbnails** : générés par **l'indexer** (phase 2, `analyzer.rs`), pas l'API. L'API crée uniquement les jobs en DB.
- **page_count = NULL** : normal après phase discovery — la phase analysis le remplit. Ne pas confondre avec une erreur.
- **Workspace Cargo** : les dépendances externes sont définies dans le `Cargo.toml` racine, pas dans les crates individuels.
- **Migrations** : dossier `infra/migrations/`, géré par sqlx. Toujours migrer avant de démarrer les services.
- **Recherche** : full-text via PostgreSQL (`ILIKE` + `pg_trgm`), pas de moteur de recherche externe.
- **Auth tokens** : format `stl_<prefix>_<secret>`, hash argon2 en DB, scopes `admin` ou `read`.

> Voir `AGENTS.md` pour les conventions de code détaillées et les patterns par module.
> Des `AGENTS.md` spécifiques existent dans `apps/api/`, `apps/indexer/`, `apps/backoffice/`, `crates/parsers/`.
