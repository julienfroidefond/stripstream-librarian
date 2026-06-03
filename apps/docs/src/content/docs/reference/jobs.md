---
title: Référence des tâches
description: Types de tâches disponibles et cycle de vie
---

## Lancer une tâche

![Page des tâches d'indexation — lanceur organisé par catégorie avec sélecteur de bibliothèque](/screenshots/jobs-launcher.png)

Depuis la page **Tâches**, sélectionnez une bibliothèque (ou Toutes) puis cliquez sur l'action souhaitée. Les tâches destructives sont signalées en rouge/orange et affichent un avertissement avant exécution.

## Types de tâches

### Indexation

| Libellé dans l'interface | Description |
|--------------------------|-------------|
| **Mise à jour** | Scan incrémental — ne revisite que les dossiers modifiés depuis le dernier scan |
| **Rescan complet** | Visite tous les dossiers sans cache — conserve toutes les données |
| **Reconstruction complète** ⚠️ | Supprime tout en base puis rescanne — métadonnées et statuts de lecture perdus |

### Miniatures

| Libellé dans l'interface | Description |
|--------------------------|-------------|
| **Générer les miniatures** | Crée les miniatures manquantes uniquement |
| **Regénérer les miniatures** ⚠️ | Supprime et recrée toutes les miniatures |

### Métadonnées

| Libellé dans l'interface | Description |
|--------------------------|-------------|
| **Métadonnées en lot** | Recherche et lie automatiquement les séries sans métadonnées approuvées |
| **Re-match métadonnées** | Supprime les liens existants et re-matche avec le provider actuel |
| **Rafraîchir métadonnées** | Met à jour les séries liées (en cours de publication uniquement) |
| **Rafraîchir toutes les métadonnées** | Met à jour toutes les séries liées, y compris terminées |

### Téléchargements

| Libellé dans l'interface | Description |
|--------------------------|-------------|
| **Détection de téléchargements** | Cherche sur Prowlarr les releases pour les volumes manquants |

### Synchronisation lecture (AniList)

| Libellé dans l'interface | Description |
|--------------------------|-------------|
| **Pull AniList** | Importe la progression depuis AniList vers Stripstream |
| **Push AniList** | Envoie la progression locale vers AniList |

## Historique des tâches

![Historique des jobs — liste filtrée par type, statut et bibliothèque avec stats et actions](/screenshots/jobs-history.png)

L'historique liste toutes les tâches avec leur type, statut, statistiques (livres ajoutés/supprimés, liens créés…), durée et date. Filtres disponibles : type, statut, bibliothèque. Chaque tâche peut être consultée (rapport détaillé) ou relancée.

## Cycle de vie

```
En attente → En cours → Terminée
                     → Échouée
                     → Annulée
```

## Suivi de progression

- Progression en temps réel (percentage, fichier en cours, compteurs)
- Rapport final avec statistiques détaillées
- Journal des erreurs non fatales (le job continue malgré elles)
- Annulation possible pour les tâches en attente ou en cours

:::note[Détails techniques]
**Types internes** :

Indexation : `rebuild` (mise à jour), `rescan` (rescan complet), `full_rebuild` (reconstruction).

Miniatures : `thumbnail_rebuild` (manquantes), `thumbnail_regenerate` (toutes), `cbr_to_cbz` (conversion).

Métadonnées : `metadata_batch`, `metadata_batch_rematch`, `metadata_refresh`, `metadata_refresh_all`.

Téléchargements : `download_detection`, `prowlarr_rss`.

AniList : `reading_status_match`, `reading_status_push`.

**Statuts intermédiaires** : `extracting_pages`, `generating_thumbnails`.

**Champs de timing** : `started_at`, `finished_at`, `phase2_started_at`.
:::
