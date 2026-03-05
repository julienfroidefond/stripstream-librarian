# Stripstream Librarian - Plan de Suivi MVP

## Objectif
Construire un serveur ultra performant pour indexer et servir des bibliotheques de livres/comics/BD (`cbz`, `cbr`, `pdf`) avec:
- API REST (librairies, livres, metadonnees, recherche, streaming pages),
- indexation incrementale,
- recherche full-text,
- admin UI (Rust SSR) pour gerer librairies/jobs/tokens,
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
- CBR: extraction temporaire disque (`unrar`) + cleanup
- Formats pages: `webp`, `jpeg`, `png`

---

## Backlog executable (ordre recommande)

### T1 - Bootstrap monorepo Rust
- [ ] Creer workspace Cargo
- [ ] Creer crates/apps: `apps/api`, `apps/indexer`, `apps/admin-ui`, `crates/core`, `crates/parsers`
- [ ] Config env centralisee + logging de base

**DoD:** Build des crates OK.

### T2 - Infra Docker Compose
- [ ] Definir services `postgres`, `meilisearch`, `api`, `indexer`
- [ ] Volumes persistants
- [ ] Healthchecks

**DoD:** `docker compose up` demarre tout, services healthy.

### T3 - Schema DB + migrations
- [ ] Tables: `libraries`, `books`, `book_files`, `index_jobs`, `api_tokens`
- [ ] Index/contraintes (uniques, FK)
- [ ] Scripts de migration

**DoD:** Migrations appliquees sans erreur, schema stable.

### T4 - Auth hybride
- [ ] Middleware `Authorization: Bearer <token>`
- [ ] Verif `API_BOOTSTRAP_TOKEN`
- [ ] Verif tokens DB (hash Argon2id, non revoques/non expires)
- [ ] MAJ `last_used_at`

**DoD:** Acces protege fonctionnel, tokens revoques refuses.

### T5 - API admin tokens
- [ ] `POST /admin/tokens` (affichage secret une seule fois)
- [ ] `GET /admin/tokens` (sans secret)
- [ ] `DELETE /admin/tokens/:id` (revoke)

**DoD:** Flux creation/liste/revocation valide.

### T6 - CRUD librairies
- [ ] `GET /libraries`
- [ ] `POST /libraries`
- [ ] `DELETE /libraries/:id`
- [ ] Validation stricte des chemins (anti traversal)

**DoD:** Gestion librairies robuste et securisee.

### T7 - Jobs d'indexation
- [ ] Orchestration scan/rebuild
- [ ] `POST /index/rebuild`
- [ ] `GET /index/status`
- [ ] Persist statuts/erreurs dans `index_jobs`

**DoD:** Jobs tracables et relancables.

### T8 - Scan incremental
- [ ] Decouverte fichiers supportes
- [ ] Fingerprint (`size`, `mtime`, hash partiel)
- [ ] Upsert `book_files`
- [ ] Detection suppressions/renommages

**DoD:** Rescan = traitement des deltas uniquement.

### T9 - Parsing CBZ
- [ ] Lecture zip + ordre pages naturel
- [ ] Metadonnees de base + cover
- [ ] Gestion erreurs archive

**DoD:** Livres CBZ indexes correctement.

### T10 - Parsing CBR (temp disk)
- [ ] Extraction temp via `unrar`
- [ ] Ordonnancement pages
- [ ] Cleanup garanti (meme en erreur)

**DoD:** Pas de fuite temp, parsing stable.

### T11 - Parsing PDF
- [ ] Metadonnees (titre/auteur/pages)
- [ ] Preparation rendu page a la volee
- [ ] Gestion fichiers corrompus

**DoD:** PDF indexes et lisibles.

### T12 - API livres
- [ ] `GET /books` (filtres + curseur)
- [ ] `GET /books/:id`
- [ ] Contrat JSON propre et stable

**DoD:** Pagination/filtres fonctionnels.

### T13 - Recherche
- [ ] Projection vers Meilisearch
- [ ] `GET /search?q=...&library_id=...&type=...`
- [ ] Fuzzy + filtres

**DoD:** Recherche rapide et pertinente.

### T14 - Streaming pages multi-format
- [ ] `GET /books/:id/pages/:n`
- [ ] Query: `format=webp|jpeg|png`, `quality=1..100`, `width`
- [ ] Headers cache (`ETag`, `Cache-Control`)
- [ ] Validation stricte params

**DoD:** Pages servies correctement dans les 3 formats.

### T15 - Perf guards
- [ ] Cache LRU en memoire (cle: `book:page:format:quality:width`)
- [ ] Limite concurrence rendu PDF
- [ ] Timeouts et bornes (`width` max)

**DoD:** Service stable sous charge homelab.

### T16 - Admin UI Rust SSR
- [ ] Vue Libraries
- [ ] Vue Jobs
- [ ] Vue API Tokens (create/list/revoke)

**DoD:** Admin complet utilisable sans SPA lourde.

### T17 - Observabilite et hardening
- [ ] Logs structures `tracing`
- [ ] Metriques Prometheus
- [ ] Health/readiness endpoints
- [ ] Rate limiting leger

**DoD:** Diagnostics et exploitation simples.

### T18 - Validation MVP
- [ ] Tests d'integration API
- [ ] Smoke tests compose
- [ ] Bench p95/p99 basiques

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
- [ ] Lot 1: Fondations (T1 -> T6)
- [ ] Lot 2: Ingestion + Search (T7 -> T13)
- [ ] Lot 3: Lecture + UI + Hardening (T14 -> T18)

## Notes
- Scope token v1: `admin`, `read`
- Bootstrap token = break-glass (peut etre desactive plus tard)
