---
title: Jobs AniList
description: Liaison des séries et synchronisation de la progression de lecture vers AniList
---

Les deux jobs AniList sont exécutés par le service **API** (job poller). Ils sont **non-exclusifs** et respectent le rate limit de l'API AniList : **700 ms entre chaque requête** (~85 req/min).

---

## `reading_status_match` — Lier les séries à AniList

Recherche chaque série de la bibliothèque sur AniList et crée automatiquement les liens dans `anilist_series_links`.

### Prérequis

AniList configuré dans les paramètres de l'application.

### Règles métier

**Sélection des séries**

- Ignore les séries nommées `"unclassified"`
- Ignore les séries déjà présentes dans `anilist_series_links` pour cette bibliothèque

**Algorithme de matching par série**

1. Recherche le nom de la série sur l'API AniList
2. Évaluation des résultats :
   - **Aucun résultat** → `anilist_no_results`
   - **1 seul résultat** → lié automatiquement, même si le titre n'est pas identique
   - **Plusieurs résultats avec match exact** (après normalisation du titre) → lié automatiquement sur le résultat exact
   - **Plusieurs résultats sans match exact** → `anilist_ambiguous` (sélection manuelle requise depuis la page série)

**Rate limit et gestion des erreurs**

- Attente fixe de **700 ms** entre chaque requête AniList
- En cas de HTTP 429 (rate limit atteint) : attente de **10 secondes** puis une tentative de reprise
- Si la reprise échoue encore → le job s'arrête avec `failed`

### Résultats par série

| `event_type` | Niveau | Signification |
|-------------|--------|---------------|
| `anilist_linked` | `info` | Série liée avec succès |
| `anilist_already_linked` | `info` | Déjà liée, ignorée |
| `anilist_no_results` | `info` | Aucun résultat AniList |
| `anilist_ambiguous` | `warning` | Plusieurs correspondances, sélection manuelle requise |
| `error` | `error` | Erreur réseau ou API |

### Rapport de job

| Champ | Signification |
|-------|--------------|
| `linked` | Séries liées avec succès |
| `already_linked` | Séries déjà liées, ignorées |
| `no_results` | Séries sans résultat AniList |
| `ambiguous` | Séries avec plusieurs correspondances |
| `errors` | Erreurs techniques |

### API

```
POST /reading-status/match
{ "library_id": "uuid" }
```

---

## `reading_status_push` — Synchroniser la progression vers AniList

Pousse la progression de lecture de chaque série liée vers AniList. Seules les séries modifiées depuis la dernière synchronisation sont envoyées (**synchronisation différentielle**).

### Prérequis

- AniList configuré avec un `local_user_id` (ID du compte AniList à mettre à jour)
- Bibliothèque configurée avec `reading_status_provider = 'anilist'`
- Liens `anilist_series_links` existants pour la bibliothèque (créés par `reading_status_match`)

### Règles métier

**Sélection différentielle**

Une série est poussée si au moins une de ces conditions est vraie :
- `synced_at IS NULL` — jamais synchronisée
- La progression de lecture d'un livre a été mise à jour depuis `synced_at`
- Un nouveau livre a été créé dans la série depuis `synced_at`

Les séries sans modification depuis le dernier push sont **ignorées**.

**Calcul du statut AniList**

| Statut AniList | Condition |
|----------------|-----------|
| `PLANNING` | 0 livre lu |
| `CURRENT` | Au moins 1 livre lu mais pas tous |
| `COMPLETED` | Tous les livres lus |

:::important
Seuls les livres avec `volume_type IN ('regular', 'integral')` sont comptés. Les hors-série (`hs`) et one-shots (`oneshot`) sont ignorés pour le calcul du statut et de la progression.
:::

Le champ `progress` envoyé à AniList = nombre de livres `regular`/`integral` marqués comme lus.

**Après un push réussi**

`anilist_series_links.synced_at` est mis à jour à `NOW()` pour cette série.

**Rate limit et gestion des erreurs**

- Attente fixe de **700 ms** entre chaque requête AniList
- En cas de HTTP 429 : attente de **10 secondes** puis une tentative de reprise
- Si la reprise échoue encore → le job s'arrête avec `failed`

### Résultats par série

| `event_type` | Niveau | Signification |
|-------------|--------|---------------|
| `status_pushed` | `info` | Progression synchronisée avec succès |
| `status_no_books` | `info` | Série sans livres, ignorée |
| `error` | `error` | Erreur réseau ou API AniList |

### Rapport de job

| Champ | Signification |
|-------|--------------|
| `pushed` | Séries synchronisées |
| `no_books` | Séries ignorées (sans livres) |
| `errors` | Erreurs techniques |

### Ordonnancement automatique

Si `reading_status_push_mode != 'manual'` et que :
- Un provider AniList est configuré sur la bibliothèque
- Des liens `anilist_series_links` existent pour la bibliothèque

…l'ordonnanceur crée automatiquement un job selon l'intervalle défini. Aucun job `reading_status_push` ne doit être déjà actif pour la bibliothèque.

### API

```
POST /reading-status/push
{ "library_id": "uuid" }
```
