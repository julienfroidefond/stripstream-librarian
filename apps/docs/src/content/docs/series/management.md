---
title: Gestion des séries
description: Organisation et métadonnées des séries
---

## Agrégation automatique

Les séries sont dérivées de la structure de répertoires pendant le scan. Les livres sans série sont groupés comme "unclassified".

## Métadonnées de série

| Champ | Description |
|-------|-------------|
| `description` | Description de la série |
| `publishers` | Éditeurs |
| `start_year` | Année de début |
| `status` | `ongoing`, `ended`, `completed`, `on_hold`, `hiatus` |
| `total_volumes` | Nombre total de tomes (provider ou manuel) |
| `authors` | Auteurs (agrégés depuis les livres ou metadata) |
| `genres` | Genres |
| `cover_url` | URL de couverture |

## Verrouillage de champs

Chaque champ peut être verrouillé individuellement pour empêcher la synchronisation metadata d'écraser les modifications manuelles.

Le verrouillage est stocké dans la colonne JSONB `locked_fields` (ex: `{"description": true}`).

## Fusion de séries

- Fusionner une série source dans une série cible
- Transfère les livres, metadata links, downloads disponibles, liens AniList
- Conserve les metadata links de la cible en cas de conflit (même provider)
- Supprime la série source après fusion
- Modal de recherche cross-library sur la page détail de série

## Nettoyage des séries orphelines

- Après suppression de livres obsolètes, les séries sans livres sont supprimées
- Les séries créées par découverte (sans livres supprimés) sont préservées
- Nettoyage supplémentaire : séries sans livres ET sans metadata links ET sans downloads

## Déduplication au renommage

`get_or_create_series` vérifie `name` et `original_name` pour éviter les doublons. Matching case-insensitive et accent-insensitive sur les deux champs.
