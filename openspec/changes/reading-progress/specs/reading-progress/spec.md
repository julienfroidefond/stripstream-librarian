## ADDED Requirements

### Requirement: Consulter la progression de lecture d'un livre
Le système SHALL retourner la progression de lecture d'un livre via `GET /books/:id/progress`. Si aucune progression n'a été enregistrée, le système SHALL retourner `{ "status": "unread", "current_page": null, "last_read_at": null }` sans erreur 404.

#### Scenario: Progression inexistante
- **WHEN** le client appelle `GET /books/:id/progress` pour un livre sans progression enregistrée
- **THEN** le système retourne HTTP 200 avec `{ "status": "unread", "current_page": null, "last_read_at": null }`

#### Scenario: Progression existante — livre en cours
- **WHEN** le client appelle `GET /books/:id/progress` pour un livre avec `status = "reading"`
- **THEN** le système retourne HTTP 200 avec `{ "status": "reading", "current_page": <n>, "last_read_at": <timestamp> }`

#### Scenario: Livre inexistant
- **WHEN** le client appelle `GET /books/:id/progress` avec un UUID invalide ou inexistant
- **THEN** le système retourne HTTP 404

### Requirement: Mettre à jour la progression de lecture
Le système SHALL permettre de mettre à jour la progression de lecture via `PATCH /books/:id/progress`. Cette route SHALL être accessible avec un token `read` ou `admin`.

#### Scenario: Marquer un livre comme lu
- **WHEN** le client envoie `PATCH /books/:id/progress` avec `{ "status": "read" }`
- **THEN** le système enregistre `status = "read"`, `current_page = null`, `last_read_at = NOW()` et retourne HTTP 200 avec la progression mise à jour

#### Scenario: Marquer un livre comme en cours avec page courante
- **WHEN** le client envoie `PATCH /books/:id/progress` avec `{ "status": "reading", "current_page": 42 }`
- **THEN** le système enregistre `status = "reading"`, `current_page = 42`, `last_read_at = NOW()` et retourne HTTP 200 avec la progression mise à jour

#### Scenario: Réinitialiser la progression
- **WHEN** le client envoie `PATCH /books/:id/progress` avec `{ "status": "unread" }`
- **THEN** le système enregistre `status = "unread"`, `current_page = null`, `last_read_at = NOW()` et retourne HTTP 200 avec la progression mise à jour

#### Scenario: current_page manquant pour status reading
- **WHEN** le client envoie `PATCH /books/:id/progress` avec `{ "status": "reading" }` sans `current_page`
- **THEN** le système retourne HTTP 422 avec un message d'erreur

#### Scenario: current_page invalide (zéro ou négatif)
- **WHEN** le client envoie `PATCH /books/:id/progress` avec `{ "status": "reading", "current_page": 0 }`
- **THEN** le système retourne HTTP 422 avec un message d'erreur

#### Scenario: current_page ignoré pour status non-reading
- **WHEN** le client envoie `PATCH /books/:id/progress` avec `{ "status": "read", "current_page": 42 }`
- **THEN** le système enregistre `current_page = null` (la valeur fournie est ignorée)

#### Scenario: Livre inexistant
- **WHEN** le client envoie `PATCH /books/:id/progress` avec un UUID de livre inexistant
- **THEN** le système retourne HTTP 404

### Requirement: last_read_at mis à jour à chaque modification
Le système SHALL mettre à jour `last_read_at` à l'horodatage courant à chaque appel `PATCH /books/:id/progress`, quel que soit le statut.

#### Scenario: last_read_at actualisé sur tout changement
- **WHEN** le client envoie `PATCH /books/:id/progress` avec n'importe quel statut valide
- **THEN** `last_read_at` dans la réponse est égal à l'heure de la requête (NOW())
