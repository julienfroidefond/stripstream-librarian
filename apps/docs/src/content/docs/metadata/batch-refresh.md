---
title: Batch & Refresh
description: Traitement en masse et rafraîchissement des métadonnées
---

## Batch (Auto-match)

Job `metadata_batch` — matcher automatiquement toutes les séries d'une bibliothèque.

- Séries traitées par `updated_at ASC` (les moins récentes en premier)
- Matching par édition pour SensCritique (mode détaillé)
- Auto-match uniquement si confiance == 1.0

### Statuts de résultat

| Statut | Description |
|--------|-------------|
| `auto_matched` | Match automatique validé |
| `no_results` | Aucun résultat trouvé |
| `too_many_results` | Trop de résultats, pas de match clair |
| `low_confidence` | Meilleur candidat sous le seuil |
| `already_linked` | Série déjà liée à un provider |

## Re-match

Job `metadata_batch_rematch` — re-lier toutes les séries à un autre provider.

- Passe les séries déjà liées au provider cible
- Supprime l'ancien lien uniquement après un nouveau match réussi
- Utilise le mode détaillé pour SensCritique

## Refresh

Job `metadata_refresh` — mettre à jour les liens approuvés avec les dernières données des providers.

### Fonctionnement

1. Re-recherche le provider, trouve le candidat correspondant
2. Compare les anciennes et nouvelles valeurs (diff par champ)
3. Met à jour les champs modifiés (même logique de sync partagée)
4. Rapports de changement détaillés par série et par livre

### Caractéristiques

- **Non-destructif** : ne met à jour que si le provider a de nouvelles données
- **Throttle** : 300ms entre les requêtes SensCritique
- **Re-matching** : après le refresh, tente de réapparier les `external_book_metadata` non liés (livres importés après le fetch initial)
- Utilise les mêmes fonctions partagées que approve et batch
