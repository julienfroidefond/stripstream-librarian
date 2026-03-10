## Why

L'application ne permet pas de suivre la progression de lecture d'un livre. Il est impossible de savoir quels livres ont été lus, lesquels sont en cours, ou de mémoriser la page courante pour reprendre là où on s'est arrêté.

## What Changes

- Nouvelle table PostgreSQL `book_reading_progress` pour stocker l'état de lecture par livre
- Nouvel endpoint `PATCH /books/:id/progress` pour mettre à jour la progression (accessible avec token `read` ou `admin`)
- Nouvel endpoint `GET /books/:id/progress` pour consulter la progression d'un livre
- Enrichissement de `GET /books/:id` avec les champs de progression (`reading_status`, `reading_current_page`, `reading_last_read_at`)
- Documentation Swagger complète via utoipa pour tous les nouveaux endpoints et schemas

## Capabilities

### New Capabilities

- `reading-progress`: Suivi de l'état de lecture d'un livre (unread / reading / read), avec mémorisation de la page courante et horodatage de la dernière lecture

### Modified Capabilities

- `book-details`: Le détail d'un livre (`GET /books/:id`) expose désormais les informations de progression de lecture

## Impact

- **DB** : nouvelle migration `0016_add_reading_progress.sql`
- **API** : nouveau module `apps/api/src/reading_progress.rs` avec 2 handlers axum
- **API** : `apps/api/src/books.rs` — `BookDetails` enrichi + requête SQL modifiée (LEFT JOIN)
- **API** : `apps/api/src/main.rs` — ajout des routes dans `read_routes` (GET) et dans une section accessible au token `read` (PATCH)
- **API** : `apps/api/src/openapi.rs` — enregistrement des nouveaux composants Swagger
