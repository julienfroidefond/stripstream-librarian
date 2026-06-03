---
title: Séries liées & Recommandations
description: Découvrir des séries similaires et des recommandations personnalisées
---

## Séries liées

Sur la page détail d'une série, un carousel **"Séries liées"** propose automatiquement des séries de votre bibliothèque partageant des points communs avec la série en cours.

Les séries sont classées selon leur proximité :

| Ce qu'elles ont en commun | Poids |
|--------------------------|-------|
| Même liste de lecture | Fort (×5 par liste commune) |
| Même auteur | Moyen (×3 par auteur commun) |
| Même genre | Léger (×2 par genre commun) |
| Même éditeur | Très léger (×1) |

Seules les séries ayant au moins un livre sont incluses.

---

## Recommandations personnalisées

Les recommandations sont basées sur votre historique de lecture. Elles apparaissent dans la page **Découverte → onglet Recommandations**.

**Comment ça marche** :
1. Stripstream prend vos dernières séries lues ou en cours de lecture (jusqu'à 5)
2. Il agrège leurs genres, auteurs et éditeurs
3. Il cherche dans votre bibliothèque les séries que vous n'avez pas encore commencées et qui correspondent à ces critères

Chaque recommandation indique pourquoi elle est suggérée — par exemple "parce que vous lisez Dragon Ball" pour une autre série du même auteur.

:::tip
Les recommandations ne s'affichent que si vous avez de l'historique de lecture. Plus vous marquez de livres comme lus, plus les suggestions sont pertinentes.
:::

Les restrictions de genre s'appliquent : les séries dont un genre est bloqué pour votre compte n'apparaissent pas dans les recommandations.

:::note[Détails techniques]
**API séries liées** :
```
GET /series/{series_id}/related?limit=10
```
`limit` : entre 1 et 50 (défaut 10). Réponse : liste de `RelatedSeriesItem` avec `score` et `match_reasons` (`same_reading_list`, `same_author`, `same_genre`, `same_publisher`).

**API recommandations** :
```
GET /series/recommendations?sources=3&limit=20
```

| Paramètre | Défaut | Max | Description |
|-----------|--------|-----|-------------|
| `sources` | 3 | 5 | Nombre de séries récentes à utiliser comme source |
| `limit` | 20 | 50 | Nombre de recommandations retournées |

Retourne `[]` si l'utilisateur n'a pas d'historique ou si le token ne porte pas d'utilisateur (admin).

Champs de réponse : `score` (score pondéré), `because_of` (noms des séries sources), `match_reasons` (`same_genre`, `same_author`, `same_publisher`).
:::
