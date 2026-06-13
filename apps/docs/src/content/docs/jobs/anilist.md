---
title: Tâches AniList
description: Liaison des séries et synchronisation de la progression de lecture vers AniList
---

Trois tâches gèrent la synchronisation avec AniList. Elles peuvent s'exécuter en parallèle avec d'autres tâches.

---

## Lier les séries à AniList

Cette tâche recherche chaque série de la bibliothèque sur AniList et crée automatiquement les correspondances. Elle est le prérequis à toute synchronisation de progression.

**Ce qui se passe** :
- Les séries déjà liées sont ignorées
- Si une seule correspondance est trouvée, elle est liée automatiquement
- Si plusieurs correspondances existent et que l'une est une correspondance exacte (même nom normalisé), elle est liée automatiquement
- Si plusieurs correspondances existent sans correspondance exacte, la série est marquée "ambiguë" et vous devez faire le choix manuellement depuis la page de la série

**Rapport** :

| Résultat | Signification |
|----------|---------------|
| Lié | Série liée avec succès |
| Déjà lié | Déjà lié, ignoré |
| Aucun résultat | Série introuvable sur AniList |
| Ambigu | Plusieurs correspondances, sélection manuelle requise |

---

## Importer les notes AniList

Récupère votre score personnel AniList pour **toutes les séries déjà liées** et l'importe comme note locale. C'est l'inverse du push : les données viennent d'AniList vers Stripstream.

**Ce qui se passe** :
- Une seule requête GraphQL récupère l'ensemble de votre liste manga AniList
- Pour chaque série localement liée, le score est converti (POINT_100 ÷ 10) et arrondi sur l'échelle 1–10
- Insertion dans `series_user_ratings` avec `ON CONFLICT DO NOTHING` — **vos notes saisies manuellement ne sont jamais écrasées**
- Si la série n'est pas dans votre liste AniList, elle est ignorée (comptabilisée comme "non trouvée")

**Rapport** :

| Résultat | Signification |
|----------|---------------|
| Mis à jour | Score importé |
| Non noté | Lié localement mais pas noté sur AniList |
| Non trouvé | Série liée localement mais absente de votre liste AniList |

:::note
Ce job ne remplace pas le bouton **Importer depuis AniList** de la page Paramètres — il couvre uniquement les séries déjà liées via "Lier les séries à AniList". Lancez d'abord ce dernier si vos séries ne sont pas encore liées.
:::

:::note[Détails techniques]
`rating_pull` — job global (sans `library_id`), exécuté par l'API. Un seul job peut tourner à la fois.  
`API : POST /ratings/pull`

Événements :
| `event_type` | Niveau |
|-------------|--------|
| `score_updated` | `info` |
| `unrated` | `info` |
| `not_found` | `info` |
:::

---

## Push vers AniList

Envoie votre progression de lecture vers AniList. Seules les séries **modifiées depuis le dernier push** sont envoyées — pas toute la bibliothèque à chaque fois.

**Ce qui est envoyé** : le statut de la série (planifié / en cours / terminé) et le nombre de tomes lus.

**Déclenchement automatique** : configurez la fréquence dans les paramètres de la bibliothèque → section **État de lecture**.

:::note
Seuls les tomes réguliers et les intégrales comptent pour la progression envoyée à AniList. Les hors-séries et one-shots sont ignorés.
:::

:::note[Détails techniques]
Les deux tâches sont exécutées par l'API (job poller), non-exclusives. Rate limit AniList : 700ms entre chaque requête (~85 req/min). Retry 10s sur HTTP 429, abandon au second 429 consécutif.

**Matching** (`reading_status_match`) :
- Ignore les séries `"unclassified"` et celles déjà dans `anilist_series_links` pour cette bibliothèque
- Correspondance exacte : après normalisation `LOWER(unaccent())` du titre
- `API : POST /reading-status/match { "library_id": "uuid" }`

Événements :
| `event_type` | Niveau |
|-------------|--------|
| `anilist_linked` | `info` |
| `anilist_already_linked` | `info` |
| `anilist_no_results` | `info` |
| `anilist_ambiguous` | `warning` |
| `error` | `error` |

**Push** (`reading_status_push`) :
- Différentiel : série poussée si `synced_at IS NULL` ou progression modifiée depuis `synced_at`
- `volume_type IN ('regular', 'integral')` uniquement pour le calcul du statut et du `progress`
- Après push réussi : `anilist_series_links.synced_at = NOW()`
- Prérequis : `local_user_id` configuré, `reading_status_provider = 'anilist'`, liens `anilist_series_links` existants
- `API : POST /reading-status/push { "library_id": "uuid" }`

Événements :
| `event_type` | Niveau |
|-------------|--------|
| `status_pushed` | `info` |
| `status_no_books` | `info` |
| `error` | `error` |
:::
