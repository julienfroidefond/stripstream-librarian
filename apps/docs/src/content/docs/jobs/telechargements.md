---
title: Tâches de détection de téléchargements
description: Détection automatique des releases disponibles via Prowlarr
---

Deux tâches complémentaires permettent de détecter des releases disponibles via Prowlarr. Elles alimentent toutes deux la même liste de volumes disponibles au téléchargement.

---

## Détection classique

La tâche de détection classique interroge Prowlarr **une fois par série** pour trouver des releases correspondant aux volumes manquants. C'est la méthode la plus précise car Prowlarr filtre directement par nom de série.

**Conditions pour qu'une série soit traitée** : la série doit avoir des métadonnées approuvées et des volumes manquants. Les séries complètes ou sans métadonnées sont ignorées.

**Déclenchement** : configurez la fréquence dans les paramètres de la bibliothèque → section **Détection de téléchargements**.

---

## Polling RSS

Le polling RSS récupère les releases récentes de **tous vos indexeurs en parallèle** (une requête par indexeur), puis les compare en mémoire à toutes vos séries avec des volumes manquants. Il est plus efficace sur un grand nombre de séries car il fait beaucoup moins de requêtes.

:::caution
Tous les indexeurs ne supportent pas le mode RSS — certains trackers privés n'acceptent que les recherches avec un terme explicite. Ces indexeurs sont ignorés silencieusement, mais leurs releases restent accessibles via la détection classique.
:::

Le polling RSS est **global** : une seule exécution couvre toutes vos bibliothèques.

**Configuration de l'intervalle** : **Settings → Download tools → Prowlarr → Polling RSS automatique**

---

## Quand utiliser chacune ?

| | Détection classique | Polling RSS |
|--|---------------------|------------|
| **Recherche** | Prowlarr filtre par nom de série | Matching local sur le titre |
| **Couverture** | Toute l'histoire de l'indexeur | Releases récentes uniquement |
| **Fréquence conseillée** | Horaire / quotidien | 30 min à 1h |
| **Périmètre** | Par bibliothèque | Toutes les bibliothèques |

En pratique, les deux tâches se complètent : le RSS pour les nouveautés en temps quasi-réel, la détection classique pour fouiller l'historique des indexeurs.

---

## Blacklist des releases

Une release peut être blacklistée pour ne plus apparaître dans les résultats futurs. Elle disparaît de la liste des volumes disponibles lors du prochain scan.

Pour gérer la blacklist : page Téléchargements → icône œil à côté du titre "Volumes disponibles".

:::note[Détails techniques]
**Détection classique** (`download_detection`) :
- Exécuté par l'API (job poller), non-exclusif
- Récupère les volumes manquants depuis `external_book_metadata` où `book_id IS NULL`
- Ignore les séries qui possèdent une intégrale (`volume_type = 'integral'`), considérées comme complètes
- Filtre la blacklist `release_blacklist` avant matching
- Fusion dans `available_downloads` : `detected_at` préservé pour les releases déjà connues
- Résultats par série : `downloads_found`, `downloads_not_found`, `no_missing_volumes`, `no_metadata`, `prowlarr_no_results`, `error`
- `API : POST /download-detection/start { "library_id": "uuid" }`

**Polling RSS** (`prowlarr_rss`) :
- Global (`library_id = NULL`), non-exclusif
- Fetch via `/api/v1/indexer` puis `/api/v1/search?query=&indexerIds=X&limit=100` en parallèle
- Normalisation des titres : points, underscores, tirets, apostrophes → espaces, accents supprimés
- Snapshot RSS (max 200 releases) stocké dans `stats_json.rss_releases`, purgé → 5 derniers jobs `success`
- Notification Telegram uniquement si `new_releases > 0`
- Résultats : `downloads_found`, `downloads_not_found`
- Champs rapport : `total_series`, `found`, `new_releases`, `rss_releases_fetched`, `rss_indexer_stats`
- `API : POST /prowlarr-rss/start { "library_id": "uuid" }`

**Blacklist** : stockée dans `release_blacklist` (titre + indexer + nom de série). Filtrée avant matching. Ne supprime pas les `available_downloads` existants — nettoyage au prochain run.

API blacklist :
| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `DELETE` | `/available-downloads/{id}?blacklist=true` | Supprimer et blacklister |
| `DELETE` | `/available-downloads/{id}` | Supprimer sans blacklister |
| `POST` | `/release-blacklist` | Blacklister manuellement |
| `DELETE` | `/release-blacklist/{id}` | Retirer de la blacklist |
| `GET` | `/release-blacklist` | Lister les releases blacklistées |
:::
