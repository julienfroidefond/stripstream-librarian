---
title: Vue d'ensemble des jobs
description: Cycle de vie, statuts, exclusivité et ordonnancement des jobs
---

Les jobs sont des tâches de fond asynchrones qui effectuent les opérations lourdes : scan de fichiers, génération de miniatures, synchronisation de métadonnées, détection de téléchargements, synchronisation AniList.

---

## Catégories de jobs

| Catégorie | Types | Exécuteur |
|-----------|-------|-----------|
| [Indexation](../indexation/) | `rebuild`, `rescan`, `full_rebuild` | Indexer |
| [Miniatures](../miniatures/) | `thumbnail_rebuild`, `thumbnail_regenerate`, `cbr_to_cbz` | Indexer |
| [Métadonnées](../metadonnees/) | `metadata_batch`, `metadata_batch_rematch`, `metadata_refresh`, `metadata_refresh_all` | API |
| [Téléchargements](../telechargements/) | `download_detection`, `prowlarr_rss` | API |
| [AniList](../anilist/) | `reading_status_match`, `reading_status_push` | API |

---

## Cycle de vie

```
pending → running → success
                 → failed
                 → cancelled
```

**Statuts intermédiaires** (phase 2 des jobs d'indexation) :

| Statut | Signification |
|--------|--------------|
| `extracting_pages` | Ouverture des archives, extraction du nombre de pages |
| `generating_thumbnails` | Génération des miniatures WebP |

---

## Jobs exclusifs vs non-exclusifs

**Exclusifs** — ne peuvent pas coexister avec un autre job exclusif sur la même bibliothèque :

- `rebuild`, `rescan`, `full_rebuild`
- `thumbnail_rebuild`, `thumbnail_regenerate`

**Non-exclusifs** — peuvent s'exécuter en parallèle :

- `cbr_to_cbz`
- `metadata_batch`, `metadata_batch_rematch`, `metadata_refresh`, `metadata_refresh_all`
- `reading_status_match`, `reading_status_push`
- `download_detection`, `prowlarr_rss`

---

## Priorité de traitement (jobs d'indexation)

| Priorité | Types |
|----------|-------|
| 1 | `full_rebuild` |
| 2 | `rebuild`, `rescan`, `scan` |
| 3 | `thumbnail_rebuild`, `thumbnail_regenerate`, `cbr_to_cbz` |

Les jobs API (`metadata_*`, `reading_status_*`, `download_detection`, `prowlarr_rss`) sont traités dans l'ordre de création (FIFO).

---

## Nettoyage des jobs bloqués

Au démarrage de l'indexer, les jobs dans un état bloqué sont automatiquement marqués `failed` :

- Jobs `running` depuis le démarrage précédent (crash/redémarrage)
- Jobs `pending` depuis plus de **30 minutes** (jamais pris en charge, probablement bloqués par un job exclusif)

---

## Ordonnancement automatique

Plusieurs types de jobs peuvent être déclenchés automatiquement selon la configuration de chaque bibliothèque :

| Job | Paramètre bibliothèque | Condition |
|-----|----------------------|-----------|
| `rebuild` / `full_rebuild` | `scan_mode` + `monitor_enabled` | `next_scan_at <= NOW()` et aucun job actif |
| `download_detection` | `download_detection_mode` | `next_download_detection_at <= NOW()`, Prowlarr configuré, aucun job actif |
| `prowlarr_rss` | *(fixe, 30 min)* | Dernier job terminé depuis > 30 min, Prowlarr configuré, aucun job actif |
| `reading_status_push` | `reading_status_push_mode` | `next_reading_status_push_at <= NOW()`, provider AniList configuré, liens AniList existants |
| `metadata_refresh` | `metadata_refresh_mode` | `next_metadata_refresh_at <= NOW()`, liens approuvés existants, aucun job actif |

La valeur `manual` désactive l'automatisation.

---

## Suivi de progression

### Server-Sent Events (SSE)

```
GET /index/jobs/{id}/stream
```

Retourne un flux d'événements en temps réel. Chaque événement contient :

```json
{
  "job_id": "...",
  "status": "running",
  "current_file": "One Piece/tome_01.cbz",
  "progress_percent": 42,
  "processed_files": 213,
  "total_files": 506,
  "stats_json": { ... }
}
```

Le flux se ferme automatiquement quand le job atteint `success`, `failed` ou `cancelled`.

### Événements de job (`index_job_events`)

Chaque action significative est tracée comme événement avec :

| Champ | Valeurs |
|-------|---------|
| `level` | `info`, `warning`, `error` |
| `event_type` | Voir la page de chaque catégorie |

```
GET /index/jobs/{id}/events?level=error&event_type=metadata_matched
GET /index/jobs/{id}/errors
```

---

## API générale des jobs

| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `GET` | `/index/status` | 100 derniers jobs |
| `GET` | `/index/jobs/active` | Jobs `pending` ou `running` |
| `GET` | `/index/jobs/{id}/details` | Détail complet d'un job |
| `GET` | `/index/jobs/{id}/events` | Événements du job |
| `GET` | `/index/jobs/{id}/errors` | Erreurs non fatales du job |
| `GET` | `/index/jobs/{id}/indexed-books` | Livres indexés pendant un scan |
| `GET` | `/index/jobs/{id}/stream` | Flux SSE de progression |
| `POST` | `/index/cancel/{id}` | Annuler un job (`pending` ou `running`) |
