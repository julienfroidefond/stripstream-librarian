---
title: Tâches de métadonnées
description: Recherche, liaison et rafraîchissement des métadonnées externes
---

Les tâches de métadonnées enrichissent vos séries avec des informations issues de providers externes. Elles peuvent toutes s'exécuter en parallèle et sont traitées dans l'ordre de création.

---

## Métadonnées en lot — Liaison automatique

Recherche et lie automatiquement toutes les séries d'une bibliothèque qui n'ont pas encore de métadonnées approuvées. C'est l'outil idéal pour enrichir une nouvelle bibliothèque en une seule opération.

**Ce qui se passe** :
- Chaque série sans lien approuvé est recherchée sur le provider configuré
- Si le résultat est évident (confiance maximale), il est approuvé automatiquement
- Sinon, la série est listée dans le rapport pour traitement manuel
- Si le provider principal ne trouve rien, le provider de secours est essayé

**Rapport de résultat** :

| Résultat | Signification |
|----------|---------------|
| **Lié automatiquement** | Match certain, approuvé sans intervention |
| **Confiance trop basse** | Trouvé, mais incertain — à valider manuellement |
| **Trop de résultats** | Plusieurs candidats, sélection manuelle requise |
| **Aucun résultat** | Aucun résultat sur le provider |
| **Déjà lié** | Métadonnées déjà approuvées, ignoré |
| **Erreur** | Échec technique |

---

## Re-match — Changer de provider en masse

Variante du batch qui ignore les liens existants et re-matche toutes les séries avec le provider actuellement configuré. Utile pour migrer une bibliothèque d'un provider à un autre sans passer manuellement sur chaque série.

Le re-match ne supprime les anciens liens qu'après avoir trouvé un nouveau match avec succès — vos données ne sont jamais perdues si le nouveau matching échoue.

---

## Rafraîchir les métadonnées

Met à jour les séries déjà liées avec les dernières données du provider. Utile pour récupérer les nouvelles sorties : nombre de tomes actualisé, couverture mise à jour, etc.

Par défaut, seules les séries **en cours de publication** sont rafraîchies — les séries terminées sont considérées stables.

Le **Refresh complet** force la mise à jour de toutes les séries liées, y compris les terminées.

Les champs verrouillés manuellement ne sont jamais modifiés par un refresh.

:::note[Détails techniques]
**Batch** (`metadata_batch`) :
- Ignore les séries nommées `"unclassified"` et celles dont la bibliothèque a `metadata_provider = 'none'`
- Résolution du provider : bibliothèque → paramètre global → fallback `google_books`
- Circuit breaker SensCritique sur HTTP 429 (annule les requêtes SensCritique restantes, pas les autres providers)
- Délai de 1s entre chaque appel provider
- `API : POST /metadata/batch { "library_id": "uuid" }`

**Re-match** (`metadata_batch_rematch`) :
- Skip uniquement si déjà lié au provider **cible** (pas aux autres)
- `API : POST /metadata/batch { "library_id": "uuid", "force_rematch": true }`

**Refresh** (`metadata_refresh`) :
- Exclut les séries avec statut `ended` ou `cancelled`
- Les séries sans statut explicite sont traitées comme `ongoing`
- Throttle 300ms entre requêtes SensCritique
- Après refresh, tente de réapparier les `external_book_metadata` non liés
- `API : POST /metadata/refresh { "library_id": "uuid" }`

**Refresh complet** (`metadata_refresh_all`) :
- Pas de filtre de statut
- `API : POST /metadata/refresh-all { "library_id": "uuid" }`

**Événements de job** :
| `event_type` | Niveau | Signification |
|-------------|--------|---------------|
| `metadata_matched` | `info` | Série liée automatiquement |
| `metadata_low_confidence` | `warning` | Confiance trop basse |
| `metadata_too_many` | `warning` | Plusieurs candidats |
| `metadata_no_results` | `info` | Aucun résultat |
| `metadata_already_linked` | `info` | Déjà lié, ignoré |
| `error` | `error` | Échec technique |
:::
