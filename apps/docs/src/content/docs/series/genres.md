---
title: Gestion des genres
description: Taguer, renommer et filtrer les séries par genre
---

Les genres sont des tags libres attachés aux séries. Ils peuvent être issus des providers de métadonnées ou définis manuellement.

## Page Genres

![Page de gestion des genres — compteurs, grille de genres avec couvertures, et navigateur de séries](/screenshots/genres-management.png)

La page **Genres** du backoffice centralise toute la gestion :

- **Grille de genres** — chaque genre affiché avec le nombre de séries associées, filtrables par bibliothèque
- **Renommage** — cliquez sur un genre pour le renommer sur toutes les séries en une fois
- **Suppression** — retire le genre de toutes les séries (les séries elles-mêmes ne sont pas supprimées)
- **Navigateur de séries** — sélectionnez un genre pour parcourir les séries qui le portent
- **Couverture de genre** — choisissez une image représentative pour chaque genre

## Taguer des séries

### Assignation en masse

Depuis la page Genres, un panneau **"Séries sans genre"** liste les séries non encore taguées. Sélectionnez-en plusieurs et assignez-leur un genre en un clic.

### Assignation individuelle

Sur la page détail d'une série, les genres sont modifiables directement dans les métadonnées.

## Filtrer les séries par genre

Sur la page **Séries**, un filtre par genre est disponible. Les séries peuvent être filtrées par un ou plusieurs genres.

## Genres et synchronisation metadata

Les genres issus des providers sont écrits lors de la synchronisation des métadonnées. Pour éviter qu'un rafraîchissement n'écrase vos genres personnalisés, verrouillez le champ **genres** sur la série.

:::note[Détails techniques]
**API Genres** :

| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `GET` | `/genres` | Liste tous les genres avec compteur de séries |
| `PATCH` | `/genres/{name}` | Renomme un genre sur toutes les séries |
| `DELETE` | `/genres/{name}` | Supprime un genre de toutes les séries |
| `POST` | `/genres/assign` | Assigne un genre à une liste de séries (bulk) |
| `GET` | `/genres/untagged-series` | Séries sans aucun genre (max 500) |

`GET /genres` et `GET /genres/untagged-series` acceptent le paramètre `library_id` (UUID) pour filtrer à une bibliothèque donnée.
:::
