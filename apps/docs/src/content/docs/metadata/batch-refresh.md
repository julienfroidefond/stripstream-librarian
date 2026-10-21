---
title: Auto-match & Refresh
description: Liaison automatique des séries non liées et rafraîchissement des métadonnées
---

## Auto-match — Séries non liées

Le job **Auto-match séries non liées** recherche et lie automatiquement les séries d'une bibliothèque qui n'ont pas encore de lien approuvé pour chaque provider configuré (principal et secondaire). C'est le moyen le plus rapide d'enrichir une nouvelle bibliothèque.

### Résultats possibles pour chaque série

Après traitement, chaque série reçoit un statut dans le rapport du job :

| Résultat | Signification |
|----------|---------------|
| **Lié automatiquement** | Un match certain a été trouvé et approuvé automatiquement |
| **Confiance trop basse** | Un résultat a été trouvé, mais il n'est pas assez certain — à valider manuellement |
| **Trop de résultats** | Plusieurs candidats plausibles, Stripstream ne peut pas choisir seul |
| **Aucun résultat** | Aucun résultat trouvé sur le provider |
| **Déjà lié** | Chaque provider configuré a déjà un lien approuvé pour cette série |

Les séries "à confiance trop basse" ou "trop de résultats" apparaissent dans le rapport — vous pouvez les traiter une par une depuis la page de chaque série.

---

## Re-match — Changer de provider

Le job **Re-match** re-matche avec le provider principal les séries qui n'y sont pas encore liées, y compris celles qui ont des liens d'autres providers. Il est utile pour migrer une bibliothèque d'un provider vers un autre (par exemple d'Open Library vers Google Books) sans repasser manuellement sur chaque série. Le provider secondaire configuré est évalué dans le même job.

Le job supprime les anciens liens uniquement après avoir trouvé un nouveau match avec succès — vos données ne sont jamais perdues si le nouveau matching échoue. Les liens du principal et du secondaire configurés sont conservés ; si l'ancien lien principal est supprimé, le lien restant devient le provider de référence.

---

## Refresh — Mettre à jour les métadonnées

Le job **Refresh** re-télécharge les données des providers pour les séries déjà liées. Il est utile pour récupérer les nouvelles sorties : quand un nouveau tome paraît, le provider l'ajoute à sa liste et le refresh met à jour votre nombre de tomes attendus.

Par défaut, le refresh ne traite que les séries encore en cours de publication — les séries terminées sont considérées stables et ignorées.

Le job **Refresh complet** force la mise à jour de toutes les séries liées, y compris les terminées.

### Ce qui est préservé

- Les champs **verrouillés** ne sont jamais modifiés par un refresh
- Le refresh est **non-destructif** : il n'efface pas vos données, il ajoute ou met à jour seulement si le provider a de nouvelles informations

:::note[Détails techniques]
**Auto-match** : job `metadata_batch`. Séries triées par `updated_at ASC` (les moins récentes en premier). SensCritique : mode détaillé via `groupProducts`. Auto-match uniquement si confidence == 1.0, pour le provider principal comme pour le secondaire. Liens vérifiés par couple série + provider. Circuit breaker SensCritique sur HTTP 429 (seules les requêtes SensCritique sont sautées). Délai de 1s entre chaque série.

**Re-match** : job `metadata_batch_rematch`. Chaque provider est évalué séparément : seul un provider déjà lié est ignoré, pas la série entière. Après un match réussi, les liens des providers non configurés sont supprimés ; le principal et le secondaire configurés sont conservés.

**Refresh** : job `metadata_refresh`. Traite uniquement les séries avec statut autre que `ended`/`cancelled`. Throttle 300ms entre les requêtes SensCritique. Après refresh, tente de réapparier les `external_book_metadata` non liés.

**Refresh complet** : job `metadata_refresh_all`. Identique à `metadata_refresh` mais sans filtre de statut.

API :
```
POST /metadata/batch
{ "library_id": "uuid" }           // auto-match
{ "library_id": "uuid", "force_rematch": true }  // re-match

POST /metadata/refresh
{ "library_id": "uuid" }

POST /metadata/refresh-all
{ "library_id": "uuid" }
```
:::
