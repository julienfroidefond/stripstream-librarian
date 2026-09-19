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
| Documentation | `apps/docs/` | 7084 | Astro / Starlight |
| PostgreSQL | infra | 6432 | — |

Crates partagés : `crates/core` (config env, paths), `crates/parsers` (CBZ/CBR/PDF/EPUB), `crates/notifications` (Telegram).

### Docker

API et Indexer partagent un seul Dockerfile (`apps/api/Dockerfile`) avec deux targets (`--target api` / `--target indexer`). Build via **cargo-chef** : stage `planner` → `recipe.json`, stage `builder` cuit les dépendances (layer cachée dans le registry), puis compile l'app. Stage `sqlx-installer` séparé pour éviter de recompiler sqlx-cli à chaque changement source. **mold** linker activé (`RUSTFLAGS`). CI via `.gitea/workflows/deploy.yml` : détection des services modifiés (`dorny/paths-filter`), build conditionnel, registry cache `mode=max`, deploy automatique.

### Metadata Providers

Providers exposés via `available_providers()` : `google_books`, `open_library`, `comicvine`, `anilist`, `bdtheque`, `bdphile`, `senscritique`. Tous implémentent le trait `MetadataProvider` (`search_series` + `get_series_books`). SensCritique utilise l'API GraphQL (`apollo.senscritique.com`) ; BDTheque, BDphile et le module `bedetheque` font du scraping HTML. Le provider `bedetheque` n'est plus exposé (site bloqué par Cloudflare) mais son code reste pour la compatibilité des liens existants.

### Indexer 2-Phase Pipeline

1. **Discovery** (`scanner.rs`) — WalkDir + metadata from filename only (zero archive I/O). Inserts books with `page_count = NULL` so they're visible immediately. Skips unchanged directories via `directory_mtimes` table.
2. **Analysis** (`analyzer.rs`) — Opens archives, extracts page count + first page, generates WebP thumbnails. Processes books where `page_count IS NULL`.

Fingerprint = SHA256(size + mtime + filename) — detects changes without re-reading files.

### Metadata Sync (shared_sync.rs)

All three sync paths (approve, batch auto-match, refresh) use the same factored code in `apps/api/src/metadata/shared_sync.rs`:
- `extract_series_fields()` — extracts fields from `metadata_json` with candidate fallback
- `upsert_series_metadata()` — INSERT ON CONFLICT with locked_fields
- `fetch_local_books()` + `match_books()` — volume matching then title containment
- `push_book_metadata()` — update local books respecting locked_fields
- Diff helpers for reporting

All providers store `description` in `metadata_json` (not just the struct field) so it survives the DB round-trip through `external_metadata_links`.

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

# Docs dev
cd apps/docs && npm install && npm run dev -- --port 7083

# Docker
./scripts/docker-push.sh              # interactive: bump version, select services, build & push

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
- `utoipa` pour la doc OpenAPI — dual spec : Client (`/openapi.json`) et Admin (`/admin/openapi.json`), UI sur `/swagger-ui` et `/admin/swagger-ui`

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
- **Caching** : `staleTimes.dynamic = 30` (client router cache), API fetches avec `{ next: { revalidate: N } }` pour les listes (15s) et données stables (30-60s). Sans `revalidate`, le défaut est `cache: "no-store"`.
- **Images** : `unoptimized: true` dans next.config — utiliser WebP pré-optimisé dans `public/` plutôt que le pipeline d'optimisation Next.js

### Documentation (Astro/Starlight)
- Site statique dans `apps/docs/`, build avec `npm run build`, servi par nginx en Docker
- Thème custom cyan/magenta dans `src/styles/custom.css`
- Contenu Markdown dans `src/content/docs/`, sidebar configurée dans `astro.config.mjs`
- Toute modification de comportement visible, de notifications, de workflow admin, ou d'écran de settings doit déclencher une mise à jour de la doc utilisateur correspondante dans `apps/docs/`, sauf demande explicite contraire.

## Tests

**Les tests sont un réflexe obligatoire.** Toute modification ou ajout de code **doit** s'accompagner de tests. Toujours lancer les tests avant de commit.

```bash
# Tests unitaires (pas de DB nécessaire)
cargo test --workspace                    # tous les tests
cargo test -p api                         # crate spécifique
cargo test -p api -- series::tests        # module spécifique
cargo test -p api -- test_name            # test unique
cargo test -- --nocapture                 # avec stdout

# Tests d'intégration DB (nécessite PostgreSQL sur localhost:6432)
DATABASE_URL="postgres://stripstream:stripstream@localhost:6432/stripstream" cargo test --workspace
```

### Tests unitaires (`#[test]`)
Pour la logique pure : parsing, matching, extraction volumes, normalisation, calculs.
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ma_fonction_edge_case() {
        assert_eq!(ma_fonction("input"), expected);
    }
}
```

### Tests d'intégration DB (`#[sqlx::test]`)
Pour tester les requêtes SQL réelles. Chaque test reçoit sa propre DB temporaire avec migrations appliquées.
```rust
#[sqlx::test(migrations = "../../infra/migrations")]
async fn series_case_insensitive(pool: sqlx::PgPool) {
    // Setup: créer les données de test
    let lib_id = Uuid::new_v4();
    sqlx::query("INSERT INTO libraries (id, name, root_path) VALUES ($1, $2, $3)")
        .bind(lib_id).bind("test").bind("/libraries/test")
        .execute(&pool).await.unwrap();
    // Test + Assert
    let id1 = get_or_create_series(&pool, lib_id, "Astérix").await.unwrap();
    let id2 = get_or_create_series(&pool, lib_id, "Asterix").await.unwrap();
    assert_eq!(id1, id2);
}
```
Le user PostgreSQL doit avoir le droit `CREATEDB` : `ALTER USER stripstream CREATEDB;`

### Quoi tester
- **Fonctions pures** : parsing volumes, matching titres, normalisation accents, dédup formats
- **Queries SQL** : matching case/accent insensitive, upserts, joins latéraux
- **Logique métier** : import torrent (expected_volumes vs replace mode), création séries (dédup), providers metadata

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
- **Series matching** : toujours `LOWER(unaccent(name))` pour comparer les noms de séries. Ne jamais matcher en exact — les torrents, providers et UI ont des casses/accents différents.
- **Next.js 16 production** : `router.replace()` ne fonctionne pas en mode standalone. Utiliser `window.history.replaceState()` + `router.refresh()` pour la navigation côté client (voir `LiveSearchForm.tsx`).
- **Discovery providers** : les providers `sc_*` (sc_trending_bd, sc_best_manga, etc.) sont normalisés vers `"senscritique"` pour les metadata links. SensCritique utilise une API GraphQL publique (`apollo.senscritique.com`).
- **Torrent import replace mode** : quand `replace_existing=true`, ne PAS filtrer par `expected_volumes` — importer tous les fichiers du torrent.
- **Volume type** : `volume_type` (`regular`, `hs`, `oneshot`, `integral`). Toute logique de matching metadata, comptage de manquants, ou progression AniList doit filtrer sur `volume_type = 'regular'` uniquement. Les HS/oneshot/integral ne participent pas à la numérotation des tomes.
- **Series extraction** : le parser utilise le **parent immédiat** du fichier comme nom de série (pas le premier répertoire). Si le parent est un sous-dossier HS/Specials/Bonus/Intégrales, il remonte d'un cran.
- **Scanner volume_type** : le scanner propage `volume_type` lors des updates (skipped-dir et fingerprint-unchanged). Un scan simple suffit à corriger les HS mal classés.
- **OpenAPI dual spec** : Client API (`/openapi.json`, read scope) et Admin API (`/admin/openapi.json`, all). Les endpoints `GET /metadata/links` et `GET /metadata/missing/:id` sont en read scope.
- **Metadata description** : tous les providers doivent stocker `description` dans `metadata_json` (pas seulement dans `candidate.description`). Sans ça, la description est perdue lors du cycle match → approve → sync car `sync_series_metadata` n'a accès qu'à `metadata_json`.
- **Duplicate torrent detection** : quand un torrent existant est trouvé par magnet hash, vérifier que `content_path` existe sur disque avant de lancer l'import. Les anciens répertoires `sl-*` sont nettoyés après import.
- **Missing books dedup** : `external_book_metadata` peut contenir des doublons par `volume_number` (éditions multiples). Les queries d'affichage utilisent `DISTINCT ON (volume_number)`.
- **Stale pending jobs** : les jobs `pending` depuis > 30 min sont marqués `failed` au cleanup (sinon ils bloquent le `NOT EXISTS` du scheduler et empêchent les prochains jobs planifiés).
- **Import count** : `ImportedFile.already_existed` distingue les fichiers réellement copiés de ceux déjà présents. Le comptage et les notifications ne comptent que les vrais nouveaux fichiers.

> Voir `AGENTS.md` pour les conventions de code détaillées et les patterns par module.
> Des `AGENTS.md` spécifiques existent dans `apps/api/`, `apps/indexer/`, `apps/backoffice/`, `crates/parsers/`.
