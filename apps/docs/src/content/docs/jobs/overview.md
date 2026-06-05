---
title: Vue d'ensemble des tâches
description: Ce que sont les tâches et comment elles s'exécutent
---

Les tâches (jobs) sont des opérations longues qui s'exécutent en arrière-plan : scan de fichiers, génération de miniatures, synchronisation de métadonnées, détection de téléchargements, synchronisation AniList.

Vous pouvez suivre leur progression en temps réel sur la page **Tâches**.

---

## Catégories de tâches

| Catégorie | Ce qu'elles font |
|-----------|-----------------|
| [Indexation](../indexation/) | Scanner les fichiers pour maintenir la bibliothèque à jour |
| [Miniatures](../miniatures/) | Générer ou regénérer les miniatures de couverture |
| [Métadonnées](../metadonnees/) | Rechercher et mettre à jour les métadonnées des séries |
| [Téléchargements](../telechargements/) | Détecter les volumes manquants via Prowlarr |
| [AniList](../anilist/) | Synchroniser la progression de lecture avec AniList |
| [Telegram Monitor](../telegram/) | Surveiller des channels Telegram pour détecter de nouveaux livres |

---

## Cycle de vie d'une tâche

Une tâche passe par les états suivants :

```
En attente → En cours → Terminée
                     → Échouée
                     → Annulée
```

Vous pouvez annuler une tâche en attente ou en cours depuis la page Tâches.

---

## Tâches exclusives et parallèles

Certaines tâches ne peuvent pas s'exécuter simultanément sur la même bibliothèque (par exemple deux scans en même temps). Si vous lancez une telle tâche alors qu'une autre est en cours, elle attend en file d'attente.

D'autres tâches (métadonnées, AniList, téléchargements) peuvent s'exécuter en parallèle sans restriction.

---

## Tâches automatiques

Plusieurs tâches peuvent être déclenchées automatiquement selon la configuration de chaque bibliothèque :

| Tâche | Se déclenche quand |
|-------|-------------------|
| Scan | Selon la fréquence configurée dans les paramètres de la bibliothèque |
| Détection de téléchargements | Selon la fréquence configurée |
| Polling RSS Prowlarr | Selon l'intervalle configuré dans Settings → Download tools |
| Push AniList | Selon la fréquence configurée par bibliothèque |
| Refresh métadonnées | Selon la fréquence configurée par bibliothèque |
| Telegram Monitor (complet) | Selon `sync_interval_minutes` dans Settings → Telegram Monitor |
| Telegram Monitor (incrémental) | Selon `sync_incremental_interval_minutes` (défaut : 30 min) |

La valeur **Manuel** désactive l'automatisation pour une tâche donnée.

---

## Nettoyage des tâches bloquées

Au démarrage, les tâches bloquées sont automatiquement marquées comme échouées :
- Tâches "en cours" depuis le redémarrage précédent (crash ou redémarrage du service)
- Tâches "en attente" depuis plus de 30 minutes (probablement bloquées par une tâche exclusive)

---

## Suivi de progression

Sur la page Tâches, chaque tâche en cours affiche :
- Pourcentage d'avancement
- Fichier en cours de traitement
- Compteurs (traités / total)
- Durée écoulée

Le rapport final de chaque tâche est consultable après son exécution, avec le détail des actions effectuées et des éventuelles erreurs.

:::note[Détails techniques]
**Statuts intermédiaires** pour les jobs d'indexation : `extracting_pages` (ouverture des archives, extraction du nombre de pages), `generating_thumbnails` (génération des miniatures WebP).

**Priorité de traitement** (Indexer) :
| Priorité | Types |
|----------|-------|
| 1 | `full_rebuild` |
| 2 | `rebuild`, `rescan`, `scan` |
| 3 | `thumbnail_rebuild`, `thumbnail_regenerate`, `cbr_to_cbz` |

Les jobs API (`metadata_*`, `reading_status_*`, `download_detection`, `prowlarr_rss`, `telegram_sync`, `telegram_sync_incremental`) sont traités en FIFO.

**Suivi temps réel** : flux SSE via `GET /index/jobs/{id}/stream`. Chaque événement contient `job_id`, `status`, `current_file`, `progress_percent`, `processed_files`, `total_files`, `stats_json`.

**API générale** :
| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `GET` | `/index/status` | 100 derniers jobs |
| `GET` | `/index/jobs/active` | Jobs pending ou running |
| `GET` | `/index/jobs/{id}/details` | Détail complet |
| `GET` | `/index/jobs/{id}/events` | Événements du job |
| `GET` | `/index/jobs/{id}/errors` | Erreurs non fatales |
| `GET` | `/index/jobs/{id}/indexed-books` | Livres indexés pendant un scan |
| `POST` | `/index/cancel/{id}` | Annuler un job |
:::
