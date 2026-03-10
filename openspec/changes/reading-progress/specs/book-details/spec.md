## MODIFIED Requirements

### Requirement: Consulter le détail d'un livre
Le système SHALL retourner les détails d'un livre via `GET /books/:id`, incluant désormais les informations de progression de lecture : `reading_status` (valeur par défaut `"unread"`), `reading_current_page` (nullable), et `reading_last_read_at` (nullable).

#### Scenario: Livre sans progression enregistrée
- **WHEN** le client appelle `GET /books/:id` pour un livre sans progression
- **THEN** le système retourne HTTP 200 avec les champs de progression à leurs valeurs par défaut : `reading_status = "unread"`, `reading_current_page = null`, `reading_last_read_at = null`

#### Scenario: Livre avec progression en cours
- **WHEN** le client appelle `GET /books/:id` pour un livre dont la progression est `reading`
- **THEN** le système retourne HTTP 200 avec `reading_status = "reading"`, `reading_current_page = <n>`, `reading_last_read_at = <timestamp>`

#### Scenario: Livre inexistant
- **WHEN** le client appelle `GET /books/:id` avec un UUID inexistant
- **THEN** le système retourne HTTP 404
