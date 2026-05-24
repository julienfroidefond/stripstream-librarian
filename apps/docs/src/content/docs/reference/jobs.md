---
title: Jobs
description: Types de jobs et cycle de vie
---

## Lancer un job

![Page des tâches d'indexation — lanceur organisé par catégorie avec sélecteur de bibliothèque](/screenshots/jobs-launcher.png)

Depuis la page **Tâches**, sélectionnez une bibliothèque (ou Toutes) puis cliquez sur l'action souhaitée. Les jobs en rouge/orange sont destructifs et affichent un avertissement.

## Types de jobs

### Indexation

| Type (interne) | Libellé UI | Description |
|----------------|-----------|-------------|
| `rebuild` | Mise à jour | Scan incrémental — ne visite que les dossiers modifiés |
| `rescan` | Rescan complet | Visite tous les dossiers sans cache mtime — sans effacer les données |
| `full_rebuild` | Reconstruction complète ⚠️ | Supprime tout en DB puis rescanne depuis zéro — métadonnées et statuts de lecture perdus |

### Miniatures

| Type (interne) | Libellé UI | Description |
|----------------|-----------|-------------|
| `thumbnail_rebuild` | Générer les miniatures | Miniatures manquantes uniquement |
| `thumbnail_regenerate` | Regénérer les miniatures ⚠️ | Recrée toutes les miniatures |

### Métadonnées

| Type (interne) | Libellé UI | Description |
|----------------|-----------|-------------|
| `metadata_batch` | Métadonnées en lot | Auto-match les séries sans lien approuvé |
| `metadata_batch_rematch` | Re-match métadonnées | Supprime les liens existants et re-matche avec le provider actuel |
| `metadata_refresh` | Rafraîchir métadonnées | Met à jour les séries déjà liées (en cours uniquement) |
| `metadata_refresh_all` | Rafraîchir toutes les métadonnées | Met à jour toutes les séries liées, y compris terminées et annulées |

### Téléchargement

| Type (interne) | Libellé UI | Description |
|----------------|-----------|-------------|
| `download_detection` | Détection de téléchargements | Cherche sur Prowlarr les releases pour les volumes manquants |

### Synchronisation lecture (AniList)

| Type (interne) | Description |
|----------------|-------------|
| `reading_status_match` | Pull — importe la progression depuis AniList |
| `reading_status_push` | Push différentiel des statuts locaux vers AniList |

## Historique des jobs

![Historique des jobs — liste filtrée par type, statut et bibliothèque avec stats et actions](/screenshots/jobs-history.png)

L'historique liste tous les jobs avec type, statut, stats (livres ajoutés/supprimés, liens créés…), durée et date. Filtres disponibles : type, statut, bibliothèque. Chaque job peut être consulté (rapport détaillé) ou rejoué.

## Cycle de vie

```
pending → running → success | failed | cancelled
```

Statuts intermédiaires : `extracting_pages`, `generating_thumbnails`

## Suivi de progression

- Progression temps réel via **Server-Sent Events** (SSE)
- Pourcentage (0–100), fichier courant, compteurs traité/total
- Timing : `started_at`, `finished_at`, `phase2_started_at`
- Blob JSON de stats spécifiques au job
- Suivi des erreurs par fichier (non-fatales : le job continue)
- Support d'annulation pour les jobs pending/running
