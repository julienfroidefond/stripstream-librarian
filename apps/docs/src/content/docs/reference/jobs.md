---
title: Jobs
description: Types de jobs et cycle de vie
---

## Types de jobs

| Type | Description |
|------|-------------|
| `rebuild` | Scan incrémental — ne visite que les dossiers modifiés (via `directory_mtimes`) |
| `full_rebuild` | Reconstruit tout depuis zéro — supprime tous les livres en DB puis rescanne le filesystem complet |
| `rescan` | Visite tous les dossiers sans se fier au cache `directory_mtimes` — utile pour forcer la relecture sans effacer les données |
| `thumbnail_rebuild` | Générer les miniatures manquantes |
| `thumbnail_regenerate` | Supprimer et régénérer toutes les miniatures |
| `cbr_to_cbz` | Convertir RAR en ZIP |
| `metadata_batch` | Auto-match séries avec metadata |
| `metadata_batch_rematch` | Re-match séries vers un autre provider |
| `metadata_refresh` | Mettre à jour les liens metadata approuvés |
| `download_detection` | Scanner Prowlarr pour volumes manquants |
| `reading_status_match` | Pull progression depuis AniList |
| `reading_status_push` | Push différentiel des statuts vers AniList |

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
