---
title: Séries liées & Recommandations
description: Découvrir des séries similaires et des recommandations personnalisées
---

## Séries liées

Sur la page détail d'une série, un carousel **"Séries liées"** propose automatiquement des séries de votre bibliothèque partageant des points communs.

### Critères de scoring

| Critère | Poids |
|---------|-------|
| Même liste de lecture | ×5 par liste commune |
| Même auteur | ×3 par auteur commun |
| Même genre | ×2 par genre commun |
| Même éditeur | ×1 |

Les séries sont triées par score décroissant, puis par nombre de livres. Seules les séries ayant au moins un livre sont incluses.

### API

```
GET /series/{series_id}/related?limit=10
```

- `limit` : entre 1 et 50 (défaut 10)
- Réponse : liste de `RelatedSeriesItem` avec `score` et `match_reasons` (`same_reading_list`, `same_author`, `same_genre`, `same_publisher`)

## Recommandations personnalisées

Les recommandations sont basées sur l'historique de lecture de l'utilisateur connecté.

### Fonctionnement

1. Les **N dernières séries** lues ou en cours de lecture sont prises comme source (défaut : 3, max : 5)
2. Leurs genres, auteurs et éditeurs sont agrégés
3. Les séries de la bibliothèque **non encore commencées** (aucun livre lu ou en cours) sont scorées selon les mêmes poids que les séries liées — les séries déjà lues ou en cours sont exclues des résultats
4. Chaque recommandation indique `because_of` : les séries sources qui ont déclenché la suggestion

### API

```
GET /series/recommendations?sources=3&limit=20
```

| Paramètre | Défaut | Max | Description |
|-----------|--------|-----|-------------|
| `sources` | 3 | 5 | Nombre de séries récentes à utiliser comme source |
| `limit` | 20 | 50 | Nombre de recommandations retournées |

Retourne `[]` si l'utilisateur n'a pas d'historique de lecture ou si le token ne porte pas d'utilisateur (admin).

> Les restrictions de genre s'appliquent : les séries dont un genre est bloqué pour l'utilisateur n'apparaissent pas dans les recommandations.

### Champs de réponse

| Champ | Description |
|-------|-------------|
| `score` | Score pondéré genres/auteurs/éditeurs |
| `because_of` | Noms des séries sources ayant déclenché la recommandation |
| `match_reasons` | Raisons : `same_genre`, `same_author`, `same_publisher` |
