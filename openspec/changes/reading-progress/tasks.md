## 1. Migration DB

- [x] 1.1 Créer `infra/migrations/0016_add_reading_progress.sql` avec la table `book_reading_progress` (book_id PK, status TEXT CHECK, current_page INT nullable, last_read_at TIMESTAMPTZ nullable, updated_at TIMESTAMPTZ)
- [x] 1.2 Ajouter un index sur `book_reading_progress(status)` pour faciliter les futurs filtres

## 2. Module reading_progress.rs

- [x] 2.1 Créer `apps/api/src/reading_progress.rs` avec les structs `ReadingProgressResponse` et `UpdateReadingProgressRequest` (avec ToSchema pour Swagger)
- [x] 2.2 Implémenter le handler `get_reading_progress` : vérifier que le livre existe (404 si non), LEFT JOIN sur `book_reading_progress`, retourner la valeur par défaut `unread` si absent
- [x] 2.3 Implémenter le handler `update_reading_progress` : vérifier que le livre existe (404 si non), valider `current_page > 0` si `status = "reading"` (422 sinon), upsert via `INSERT ... ON CONFLICT DO UPDATE`
- [x] 2.4 Ajouter les annotations `#[utoipa::path(...)]` sur les deux handlers (tag "reading-progress", params, request body, responses 200/404/422/401, security Bearer)

## 3. Enrichissement GET /books/:id

- [x] 3.1 Dans `apps/api/src/books.rs`, ajouter les champs `reading_status`, `reading_current_page`, `reading_last_read_at` à la struct `BookDetails` (avec ToSchema)
- [x] 3.2 Modifier la requête SQL de `get_book` pour inclure un `LEFT JOIN book_reading_progress brp ON brp.book_id = b.id` et mapper les champs (valeur par défaut `"unread"` via COALESCE)

## 4. Routes dans main.rs

- [x] 4.1 Déclarer le module `reading_progress` dans `apps/api/src/main.rs`
- [x] 4.2 Ajouter `GET /books/:id/progress` dans `read_routes`
- [x] 4.3 Ajouter `PATCH /books/:id/progress` dans `read_routes` (accessible token read et admin)

## 5. Swagger / OpenAPI

- [x] 5.1 Dans `apps/api/src/openapi.rs`, enregistrer `ReadingProgressResponse`, `UpdateReadingProgressRequest` dans les `components(schemas(...))`
- [x] 5.2 Enregistrer les paths `get_reading_progress` et `update_reading_progress` dans `paths(...)`
- [x] 5.3 Vérifier que `BookDetails` mis à jour est correctement reflété dans le Swagger généré
