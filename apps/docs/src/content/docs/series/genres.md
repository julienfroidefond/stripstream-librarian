---
title: Gestion des genres
description: Taguer, renommer et filtrer les séries par genre
---

Les genres sont des tags libres attachés aux séries. Ils peuvent être issus des providers de métadonnées ou définis manuellement.

## Page Genres

La page `/genres` du backoffice centralise toute la gestion :

- **Grille de genres** — chaque genre affiché sous forme de pill avec le nombre de séries associées, filtrables par bibliothèque
- **Renommage inline** — cliquez sur un genre pour le renommer sur toutes les séries en une fois
- **Suppression** — retire le genre de toutes les séries (les séries ne sont pas supprimées)
- **Navigateur de séries** — sélectionnez un genre pour parcourir les séries qui le portent, avec filtre bibliothèque
- **Sélection de couverture** — choisissez une couverture représentative par genre

## Taguer des séries

### Assignation en masse

Depuis la page Genres, un panneau "Séries sans genre" liste les séries non encore taguées (filtrables par bibliothèque). Sélectionnez-en plusieurs et assignez-leur un genre en un clic.

### Assignation individuelle

Sur la page détail d'une série, les genres sont modifiables directement dans les métadonnées.

## API

| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `GET` | `/genres` | Liste tous les genres avec compteur de séries |
| `PATCH` | `/genres/{name}` | Renomme un genre sur toutes les séries |
| `DELETE` | `/genres/{name}` | Supprime un genre de toutes les séries |
| `POST` | `/genres/assign` | Assigne un genre à une liste de séries (bulk) |
| `GET` | `/genres/untagged-series` | Séries sans aucun genre (max 500) |

### Paramètres

`GET /genres` accepte `library_id` (UUID) pour filtrer les compteurs à une bibliothèque donnée.

`GET /genres/untagged-series` accepte également `library_id`.

## Filtrer les séries par genre

Sur la page Séries, un filtre par genre est disponible. Les séries peuvent être filtrées par un ou plusieurs genres — les compteurs tiennent compte de la bibliothèque sélectionnée.

## Genres et synchronisation metadata

Les genres issus des providers sont écrits lors de la synchronisation metadata. Pour éviter qu'un refresh n'écrase vos genres manuels, verrouillez le champ `genres` sur la série.
