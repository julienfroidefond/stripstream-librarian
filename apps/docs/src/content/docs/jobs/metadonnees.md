---
title: Jobs de métadonnées
description: Recherche, liaison et rafraîchissement des métadonnées externes
---

Les jobs de métadonnées sont exécutés par le service **API** (job poller). Ils sont tous **non-exclusifs** et traités en FIFO.

---

## `metadata_batch` — Liaison automatique en lot

Recherche et lie automatiquement toutes les séries d'une bibliothèque qui n'ont pas encore de lien approuvé.

### Règles métier

**Sélection des séries traitées**

- Traite uniquement les séries **sans lien approuvé** (quel que soit le provider)
- Ignore les séries nommées `"unclassified"`
- Refuse si la bibliothèque a `metadata_provider = 'none'`

**Résolution du provider**

1. `metadata_provider` de la bibliothèque
2. Paramètre global de l'application
3. Fallback : `google_books`

Si un `fallback_metadata_provider` est configuré sur la bibliothèque, il est essayé en second si le provider principal ne retourne rien.

**Algorithme par série**

1. Recherche sur le provider principal
2. Si aucun résultat → recherche sur le provider fallback (si configuré)
3. Évaluation du résultat :
   - 1 résultat avec confidence ≥ seuil → **`auto_matched`** (lien auto-approuvé)
   - 1 résultat avec confidence < seuil → **`low_confidence`** (en attente de validation manuelle)
   - Plusieurs résultats → **`too_many_results`** (ambigu)
   - Aucun résultat → **`no_results`**
   - Déjà lié → **`already_linked`** (sauté)
   - Erreur → **`error`**

**Circuit breaker SensCritique**

Si le provider SensCritique répond avec HTTP 429 (rate limit), toutes les requêtes SensCritique restantes du job sont annulées. Les autres providers continuent normalement.

**Délai entre requêtes**

1 seconde entre chaque appel provider pour éviter le rate limiting.

### Rapport de job

| Champ | Signification |
|-------|--------------|
| `auto_matched` | Liens créés et approuvés automatiquement |
| `low_confidence` | Trouvé mais confidence trop basse (revue manuelle) |
| `too_many_results` | Plusieurs résultats, ambigus |
| `no_results` | Aucun résultat provider |
| `already_linked` | Déjà lié, sauté |
| `errors` | Erreurs techniques |

### API

```
POST /metadata/batch
{ "library_id": "uuid" }   // optionnel
```

---

## `metadata_batch_rematch` — Re-match forcé

Variante de `metadata_batch` qui ignore les liens existants et re-matche tout depuis zéro avec le provider cible.

### Règles métier — différences par rapport à `metadata_batch`

- **Skip uniquement** si la série est déjà liée au provider **cible** (pas aux autres providers)
- Si un nouveau lien est créé sur un provider différent de l'ancien : **supprime les anciens liens** des autres providers
- Permet de migrer une bibliothèque d'un provider à un autre sans repasser manuellement sur chaque série

### Cas d'usage

Migrer toutes les séries d'`open_library` vers `google_books`, ou re-matcher après avoir changé le provider par défaut de la bibliothèque.

### API

```
POST /metadata/batch
{ "library_id": "uuid", "force_rematch": true }
```

---

## `metadata_refresh` — Rafraîchir les métadonnées (séries en cours)

Met à jour les métadonnées des séries déjà liées dont le statut est `ongoing` (en cours).

### Règles métier

- Traite uniquement les séries avec un **lien approuvé**
- **Exclut** les séries avec statut `ended` ou `cancelled` — elles sont considérées stables
- Les séries sans statut explicite sont traitées comme `ongoing`
- Respecte les `locked_fields` : les champs verrouillés manuellement ne sont pas écrasés
- Met à jour : couverture, description, auteurs, statut, nombre de volumes, liste des tomes
- Non-exclusif, planifiable automatiquement

### Ordonnancement automatique

Si `metadata_refresh_mode != 'manual'` et qu'il existe des liens approuvés, l'ordonnanceur crée un job `metadata_refresh` selon l'intervalle configuré.

### API

```
POST /metadata/refresh
{ "library_id": "uuid" }
```

---

## `metadata_refresh_all` — Rafraîchir toutes les métadonnées

Comme `metadata_refresh` mais **sans filtre de statut** : rafraîchit aussi les séries `ended` et `cancelled`.

### Règles métier

- Traite toutes les séries avec un lien approuvé, quel que soit leur statut
- Même respect des `locked_fields` que `metadata_refresh`
- Non-exclusif

### Cas d'usage

Après une correction de provider ou une correction de données en masse, pour s'assurer que toutes les séries (y compris terminées) ont les métadonnées à jour.

### API

```
POST /metadata/refresh-all
{ "library_id": "uuid" }
```

---

## Événements de job

| `event_type` | Niveau | Signification |
|-------------|--------|---------------|
| `metadata_matched` | `info` | Série liée automatiquement |
| `metadata_low_confidence` | `warning` | Trouvé, confidence trop basse |
| `metadata_too_many` | `warning` | Plusieurs candidats, ambigu |
| `metadata_no_results` | `info` | Aucun résultat |
| `metadata_already_linked` | `info` | Déjà lié, ignoré |
| `error` | `error` | Échec de recherche ou d'application |
