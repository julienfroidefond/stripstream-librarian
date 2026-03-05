# Stripstream Librarian - Plan de Suivi MVP

## Objectif
Construire un serveur ultra performant pour indexer et servir des bibliotheques de livres/comics/BD (`cbz`, `cbr`, `pdf`) avec:
- API REST (librairies, livres, metadonnees, recherche, streaming pages),
- indexation incrementale,
- recherche full-text,
 - backoffice web (Next.js) pour gerer librairies/jobs/tokens,
- deploiement Docker Compose homelab.

## Decisions figees
- Backend/API: Rust (`axum`)
- Indexation: service Rust dedie (`indexer`)
- DB: PostgreSQL
- Recherche: Meilisearch
- Deploiement: Docker Compose
- Auth: token bootstrap env + tokens admin en DB (creables/revocables)
- Expiration tokens admin: aucune par defaut (revocation manuelle)
- Rendu PDF: a la volee
- CBR: extraction temporaire disque (`unrar-free`, commande `unrar`) + cleanup
- Formats pages: `webp`, `jpeg`, `png`
- Backoffice UI: Next.js (remplace Rust SSR)

---

## Backlog executable (ordre recommande)

### T1 - Bootstrap monorepo Rust
- [x] Creer workspace Cargo
- [x] Creer crates/apps: `apps/api`, `apps/indexer`, `apps/admin-ui`, `crates/core`, `crates/parsers`
- [x] Config env centralisee + logging de base

**DoD:** Build des crates OK.

### T2 - Infra Docker Compose
- [x] Definir services `postgres`, `meilisearch`, `api`, `indexer`
- [x] Volumes persistants
- [x] Healthchecks

**DoD:** `docker compose up` demarre tout, services healthy.

### T3 - Schema DB + migrations
- [x] Tables: `libraries`, `books`, `book_files`, `index_jobs`, `api_tokens`
- [x] Index/contraintes (uniques, FK)
- [x] Scripts de migration

**DoD:** Migrations appliquees sans erreur, schema stable.

### T4 - Auth hybride
- [x] Middleware `Authorization: Bearer <token>`
- [x] Verif `API_BOOTSTRAP_TOKEN`
- [x] Verif tokens DB (hash Argon2id, non revoques/non expires)
- [x] MAJ `last_used_at`

**DoD:** Acces protege fonctionnel, tokens revoques refuses.

### T5 - API admin tokens
- [x] `POST /admin/tokens` (affichage secret une seule fois)
- [x] `GET /admin/tokens` (sans secret)
- [x] `DELETE /admin/tokens/:id` (revoke)

**DoD:** Flux creation/liste/revocation valide.

### T6 - CRUD librairies
- [x] `GET /libraries`
- [x] `POST /libraries`
- [x] `DELETE /libraries/:id`
- [x] Validation stricte des chemins (anti traversal)

**DoD:** Gestion librairies robuste et securisee.

### T7 - Jobs d'indexation
- [x] Orchestration scan/rebuild
- [x] `POST /index/rebuild`
- [x] `GET /index/status`
- [x] Persist statuts/erreurs dans `index_jobs`

**DoD:** Jobs tracables et relancables.

### T8 - Scan incremental
- [x] Decouverte fichiers supportes
- [x] Fingerprint (`size`, `mtime`, hash partiel)
- [x] Upsert `book_files`
- [x] Detection suppressions/renommages

**DoD:** Rescan = traitement des deltas uniquement.

### T9 - Parsing CBZ
- [x] Lecture zip + ordre pages naturel
- [x] Metadonnees de base + cover
- [x] Gestion erreurs archive

**DoD:** Livres CBZ indexes correctement.

### T10 - Parsing CBR (temp disk)
- [x] Extraction temp via `unrar`
- [x] Ordonnancement pages
- [x] Cleanup garanti (meme en erreur)

**DoD:** Pas de fuite temp, parsing stable.

### T11 - Parsing PDF
- [x] Metadonnees (titre/auteur/pages)
- [x] Preparation rendu page a la volee
- [x] Gestion fichiers corrompus

**DoD:** PDF indexes et lisibles.

### T12 - API livres
- [x] `GET /books` (filtres + curseur)
- [x] `GET /books/:id`
- [x] Contrat JSON propre et stable

**DoD:** Pagination/filtres fonctionnels.

### T13 - Recherche
- [x] Projection vers Meilisearch
- [x] `GET /search?q=...&library_id=...&type=...`
- [x] Fuzzy + filtres

**DoD:** Recherche rapide et pertinente.

### T14 - Streaming pages multi-format
- [x] `GET /books/:id/pages/:n`
- [x] Query: `format=webp|jpeg|png`, `quality=1..100`, `width`
- [x] Headers cache (`ETag`, `Cache-Control`)
- [x] Validation stricte params

**DoD:** Pages servies correctement dans les 3 formats.

### T15 - Perf guards
- [x] Cache LRU en memoire (cle: `book:page:format:quality:width`)
- [x] Limite concurrence rendu PDF
- [x] Timeouts et bornes (`width` max)

**DoD:** Service stable sous charge homelab.

### T16 - Backoffice Next.js
- [x] Bootstrap app Next.js (`apps/backoffice`)
- [x] Vue Libraries
- [x] Vue Jobs
- [x] Vue API Tokens (create/list/revoke)
- [x] Auth backoffice via token API

**DoD:** Backoffice Next.js utilisable pour l'administration complete.

### T17 - Observabilite et hardening
- [x] Logs structures `tracing`
- [x] Metriques Prometheus
- [x] Health/readiness endpoints
- [x] Rate limiting leger

**DoD:** Diagnostics et exploitation simples.

### T18 - Validation MVP
- [ ] Tests d'integration API
- [x] Smoke tests compose
- [x] Bench p95/p99 basiques

**DoD:** Checklist MVP validee de bout en bout.

---

## Contrat API minimum (v1)
- `GET /libraries`
- `POST /libraries`
- `DELETE /libraries/:id`
- `GET /books`
- `GET /books/:id`
- `GET /search`
- `GET /books/:id/pages/:n`
- `POST /index/rebuild`
- `GET /index/status`
- `POST /admin/tokens`
- `GET /admin/tokens`
- `DELETE /admin/tokens/:id`

---

## Cibles perf MVP (homelab)
- `GET /books/:id` < 30 ms p95 (cache chaud)
- `GET /search` < 80 ms p95
- 1ere reponse `/pages/:n` < 200 ms p95 (hors PDF froid lourd)

---

## Risques connus et mitigation
- CBR heterogene -> isolation erreurs + logs detailles + retries limites
- PDF lourds CPU -> pool borne + timeout + cache
- Drift DB/Search -> indexation idempotente + job rebuild complet

---

## Suivi d'avancement
- [x] Lot 1: Fondations (T1 -> T6)
- [x] Lot 2: Ingestion + Search (T7 -> T13)
- [ ] Lot 3: Lecture + UI + Hardening (T14 -> T18)

## Notes
- Scope token v1: `admin`, `read`
- Bootstrap token = break-glass (peut etre desactive plus tard)

## Journal
- 2026-03-05: `docker compose up -d --build` valide, stack complete en healthy (`postgres`, `meilisearch`, `api`, `indexer`, `admin-ui`).
- 2026-03-05: ajustements infra appliques pour demarrage stable (`unrar` -> `unrar-free`, image `rust:1-bookworm`, healthchecks `127.0.0.1`).
- 2026-03-05: ajout d'un service `migrate` dans Compose pour executer automatiquement `infra/migrations/0001_init.sql` au demarrage.
- 2026-03-05: Lot 2 termine (jobs, scan incremental, parsers `cbz/cbr/pdf`, API livres, sync + recherche Meilisearch).
- 2026-03-05: verification de bout en bout OK sur une librairie de test (`/libraries/demo`) avec indexation, listing `/books` et recherche `/search` (1 CBZ detecte).
- 2026-03-05: Lot 3 avancee: endpoint pages (`/books/:id/pages/:n`) actif avec cache LRU, ETag/Cache-Control, limite concurrence rendu et timeouts.
- 2026-03-05: hardening API: readiness expose sans auth via `route_layer`, metriques simples `/metrics`, rate limiting lecture (120 req/s).
- 2026-03-05: smoke + bench scripts corriges et verifies (`infra/smoke.sh`, `infra/bench.sh`).
- 2026-03-05: pivot backoffice valide: remplacement de l'admin UI Rust SSR par une app Next.js.
- 2026-03-05: backoffice Next.js implemente (`apps/backoffice`) avec branding neon base sur le logo, actions libraries/jobs/tokens, et integration Docker Compose.
