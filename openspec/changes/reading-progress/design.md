## Context

L'API est une application axum single-user protégée par tokens (`admin` / `read`). Les données sont dans PostgreSQL, gérées via sqlx. Les routes existantes sont groupées en `admin_routes` et `read_routes` dans `main.rs`. La table `books` stocke uniquement les métadonnées de contenu (titre, auteur, pages, etc.) sans aucune notion d'état de lecture.

Le `GET /books/:id` utilise déjà un `LEFT JOIN LATERAL` pour les `book_files` — le même pattern s'applique naturellement pour la progression de lecture.

## Goals / Non-Goals

**Goals:**
- Stocker et exposer l'état de lecture (unread / reading / read) par livre
- Mémoriser la page courante pour reprendre la lecture
- Horodater la dernière activité de lecture
- Enrichir `GET /books/:id` sans breaking change
- Documenter tous les endpoints dans Swagger via utoipa

**Non-Goals:**
- Support multi-utilisateur (pas de concept d'utilisateur dans l'app)
- Historique des sessions de lecture
- Synchronisation entre clients
- Progression sur `GET /books` (liste paginée — impact perf non justifié)

## Decisions

### D1 — Table séparée `book_reading_progress` (vs colonnes sur `books`)

`book_reading_progress` avec `book_id` comme PRIMARY KEY (relation 1-to-1).

**Rationale** : sépare les métadonnées de contenu (immuables, issues de l'indexer) des données de lecture (mutables, issues de l'utilisateur). Facilite les requêtes ciblées et isole les permissions futures si multi-user.

**Alternative rejetée** : colonnes directes sur `books` — plus simple mais mélange deux responsabilités différentes dans la même table.

### D2 — Upsert sur PATCH (INSERT ... ON CONFLICT DO UPDATE)

Une seule ligne par livre, créée à la première mise à jour. Pas de `created_at` dans la table.

**Rationale** : évite un GET + INSERT/UPDATE en deux temps. La sémantique "créer si absent" est transparente pour le client.

### D3 — Token `read` autorisé à écrire la progression

Le PATCH est ajouté dans une section de `main.rs` protégée par `require_read` (pas `require_admin`).

**Rationale** : le cas d'usage principal est une app de lecture tournant avec un token read-only. Exiger un token admin serait inutilement contraignant pour une opération purement personnelle.

### D4 — Réponse par défaut si progression inexistante

`GET /books/:id/progress` retourne `{ "status": "unread", "current_page": null, "last_read_at": null }` si aucune ligne n'existe, plutôt qu'un 404.

**Rationale** : simplifie les clients — pas besoin de gérer un 404 comme état "non lu". L'absence de progression EST l'état "unread".

### D5 — `current_page` : valeur positive sans validation contre `page_count`

Accepter toute valeur `> 0` sans vérifier que la page existe dans le livre.

**Rationale** : `page_count` peut être NULL (phase d'analyse pas encore terminée). La validation côté client est plus appropriée.

## Risks / Trade-offs

- **Pas de validation de page** → un client peut enregistrer `current_page = 9999` sur un livre de 10 pages. Risque faible dans un contexte single-user personnel.
- **Token read en écriture** → légère déviation du principe least-privilege. Acceptable car la progression n'affecte pas les autres données.
- **LEFT JOIN dans `GET /books/:id`** → requête légèrement plus lourde. Impact négligeable (1 row lookup sur clé primaire).

## Migration Plan

1. Déployer la migration `0016_add_reading_progress.sql` (non-destructive, nouvelle table)
2. Déployer le binaire API avec les nouveaux endpoints
3. Rollback : supprimer la table et redéployer l'ancienne version (aucune donnée critique perdue)
