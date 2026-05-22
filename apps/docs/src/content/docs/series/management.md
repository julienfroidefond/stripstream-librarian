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

## Wishlist

Les séries **sans livres** fonctionnent comme une wishlist — elles ont été ajoutées via la découverte mais pas encore importées dans la bibliothèque.

Sur la page Séries, un filtre permet de basculer entre trois états :

| Valeur | Description |
|--------|-------------|
| (tous) | Toutes les séries |
| Wishlist | Séries sans livres (ajoutées en wishlist) |
| Dans la bibliothèque | Séries avec au moins un livre |

### API

Le paramètre `no_books=true` retourne uniquement les séries sans livres. Le paramètre `has_books=true` retourne uniquement les séries avec au moins un livre. Les deux sont mutuellement exclusifs.

### Nettoyage automatique

Les séries sans livres **et** sans metadata links **et** sans downloads disponibles sont supprimées automatiquement lors du nettoyage des séries orphelines. Les séries ajoutées via la découverte (avec metadata links) sont préservées.
