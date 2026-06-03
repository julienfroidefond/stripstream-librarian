---
title: Jobs de détection de téléchargements
description: Détection automatique des releases disponibles via Prowlarr — détection classique et polling RSS
---

Deux jobs complémentaires permettent de détecter des releases disponibles via Prowlarr. Tous deux sont exécutés par le service **API** (job poller), sont **non-exclusifs**, et alimentent la même table `available_downloads`.

---

## `download_detection` — Détection classique

### Prérequis

- **Prowlarr configuré** dans les paramètres de l'application (URL + clé API)
- La vérification de configuration est faite **avant** de créer le job ; si Prowlarr n'est pas configuré, l'appel retourne une erreur immédiatement

### Règles métier

**Sélection des séries traitées**

Pour chaque série de la bibliothèque avec un **lien metadata approuvé** :

1. Récupère les **volumes manquants** depuis `external_book_metadata` où `book_id IS NULL` (volume connu du provider mais absent en local)
2. Si aucun volume manquant → résultat `no_missing_volumes` (série complète ou non liée)
3. Si pas de lien approuvé → résultat `no_metadata` (skip)

**Recherche Prowlarr**

Pour chaque série avec des volumes manquants :

1. Envoie une requête de recherche à Prowlarr avec le nom de la série
2. Filtre les releases présentes dans la **blacklist** (`release_blacklist`)
3. Pour chaque release non blacklistée, tente de matcher les numéros de volumes dans le titre de la release
4. Calcule `matched_missing_volumes` : liste des volumes manquants couverts par cette release
5. Calcule `all_volumes` : tous les volumes détectés dans le titre de la release

**Marquage `has_failed`**

Si des volumes de la release ont déjà fait l'objet d'un téléchargement échoué pour cette série, la release est marquée `has_failed = true` dans l'affichage (mais reste disponible).

**Fusion des résultats**

Les résultats sont fusionnés avec ceux des runs précédents dans `available_downloads` :
- Si une release existait déjà → son `detected_at` original est **préservé**
- Si une release est nouvelle → `detected_at` = heure de début du job
- `new_releases` = nombre de releases dont `detected_at >= job_started_at`

**Nettoyage post-job**

Après le job, les entrées `available_downloads` dont la liste de releases est vide sont supprimées.

### Résultats par série

| `event_type` | Signification |
|-------------|---------------|
| `downloads_found` | Des releases correspondantes ont été trouvées |
| `downloads_not_found` | Prowlarr n'a retourné aucune release utilisable |
| `no_missing_volumes` | Série complète, rien à chercher |
| `no_metadata` | Pas de lien metadata approuvé |
| `prowlarr_no_results` | Prowlarr n'a rien retourné (indexer ou réseau) |
| `error` | Erreur technique |

### Rapport de job

| Champ | Signification |
|-------|--------------|
| `total_series` | Séries traitées |
| `found` | Séries pour lesquelles des releases ont été trouvées |
| `new_releases` | Nouvelles releases détectées lors de ce run |
| `not_found` | Séries sans résultat Prowlarr |
| `no_missing` | Séries complètes |
| `no_metadata` | Séries sans lien approuvé |
| `errors` | Erreurs techniques |

---

### Ordonnancement automatique

Si `download_detection_mode != 'manual'` et que Prowlarr est configuré, l'ordonnanceur crée automatiquement un job selon l'intervalle défini dans les paramètres de la bibliothèque.

Condition supplémentaire : aucun job `download_detection` ne doit être déjà actif (`pending` ou `running`) pour cette bibliothèque.

---

## `prowlarr_rss` — Polling RSS

### Principe

Au lieu de faire **N requêtes Prowlarr** (une par série), le job `prowlarr_rss` récupère les releases récentes de **chaque indexer Prowlarr en parallèle** (query vide, RSS-style), déduplique les résultats par GUID, puis les rapproche en mémoire de **toutes les bibliothèques** ayant des séries avec des volumes manquants.

Le job est **global** (`library_id = NULL`) : une seule instance couvre toutes les bibliothèques.

| | `download_detection` | `prowlarr_rss` |
|--|---------------------|----------------|
| Requêtes Prowlarr | 1 par série | 1 par indexer (parallèle) |
| Correspondance | Prowlarr filtre par nom | Matching local sur le titre |
| Couverture | Toute l'histoire de l'indexer | Releases récentes uniquement |
| Périmètre | Par bibliothèque | Toutes les bibliothèques |
| Fréquence conseillée | Horaire / quotidien | 30 min à 1h |

:::caution
**Tous les indexers ne supportent pas le mode RSS.** Certains trackers (notamment les trackers privés comme C411) n'acceptent que les requêtes avec un terme de recherche explicite et retournent une erreur pour les queries vides. Ces indexers sont ignorés silencieusement — leurs releases restent accessibles via `download_detection`. Le rapport du job liste le résultat par indexer dans `rss_indexer_stats`.
:::

### Prérequis

Identiques à `download_detection` : Prowlarr configuré, séries avec un **lien metadata approuvé** et des volumes manquants.

### Règles métier

**Fetch des releases**

La liste des indexers est d'abord récupérée via `/api/v1/indexer`. Une requête est ensuite envoyée **en parallèle** à chaque indexer (`/api/v1/search?query=&indexerIds=X&limit=100`) avec les catégories configurées. Les résultats sont agrégés et dédupliqués par GUID. Les indexers qui ne supportent pas les queries vides (trackers privés) retournent une erreur et sont simplement ignorés.

**Matching titre / série**

Pour chaque série (toutes bibliothèques confondues) avec des volumes manquants, le titre de chaque release est normalisé puis comparé au nom de la série :

- Points, underscores, tirets, apostrophes, crochets → espaces
- Accents courants supprimés (`é→e`, `à→a`, etc.)
- Comparaison insensible à la casse

Si le titre normalisé **contient** le nom normalisé de la série, `match_title_volumes` est appliqué pour identifier les volumes couverts.

**Résultats et fusion**

Identiques à `download_detection` : upsert dans `available_downloads` via `merge_releases()`, `detected_at` préservé pour les releases déjà connues.

**Snapshot du flux RSS**

Les releases brutes retournées par Prowlarr (max 200) sont stockées dans `stats_json.rss_releases` pour consultation dans le backoffice. Ce snapshot est **purgé automatiquement** : seuls les 5 derniers jobs `success` le conservent — les jobs plus anciens gardent uniquement les stats agrégées.

**Notification Telegram**

Une notification est envoyée uniquement si `new_releases > 0` (nouvelles releases détectées ce run), pour éviter le spam sur les polls sans résultat.

### Résultats par série

| `event_type` | Signification |
|-------------|---------------|
| `downloads_found` | Des releases correspondantes ont été trouvées dans le flux RSS |
| `downloads_not_found` | Aucune release ne correspond à cette série dans le flux courant |

### Rapport de job

| Champ | Signification |
|-------|--------------|
| `total_series` | Séries avec volumes manquants vérifiées (toutes bibliothèques) |
| `found` | Séries pour lesquelles des releases ont été trouvées |
| `new_releases` | Nouvelles releases détectées lors de ce run |
| `rss_releases_fetched` | Nombre total de releases agrégées depuis tous les indexers |
| `rss_indexer_stats` | Détail par indexer : `id`, `name`, `count` (releases retournées), `error` si indisponible |

### Ordonnancement automatique

L'intervalle est configurable dans **Settings → Download tools → Prowlarr → Polling RSS automatique** :

| Option | Valeur |
|--------|--------|
| Désactivé | Aucun job automatique |
| Toutes les 30 minutes | (défaut) |
| Toutes les heures | |
| Toutes les 6 / 12 heures | |
| Une fois par jour / semaine | |

Conditions pour déclencher un job :
- Prowlarr configuré
- Intervalle ≠ 0 (non désactivé)
- Aucun job `prowlarr_rss` global déjà actif (`pending` ou `running`)
- Le dernier job `prowlarr_rss` global terminé date de plus de l'intervalle configuré

### API

```
POST /prowlarr-rss/start
{ "library_id": "uuid" }   // optionnel — job global si absent
```

---

## Blacklist des releases

Une release peut être blacklistée pour ne plus apparaître dans les résultats futurs.

### Règles

- La blacklist est stockée dans `release_blacklist` (titre + indexer + nom de série)
- Les releases blacklistées sont filtrées **avant** le matching volumes, dans chaque run
- Blacklister une release **ne supprime pas** les entrées `available_downloads` existantes pour cette release (elles sont nettoyées au prochain run)

### API

| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `DELETE` | `/available-downloads/{id}?blacklist=true` | Supprimer et blacklister |
| `DELETE` | `/available-downloads/{id}` | Supprimer sans blacklister |
| `POST` | `/release-blacklist` | Blacklister manuellement |
| `DELETE` | `/release-blacklist/{id}` | Retirer de la blacklist |
| `GET` | `/release-blacklist` | Lister les releases blacklistées |

---

## API

```
POST /download-detection/start
{ "library_id": "uuid" }   // optionnel — toutes les bibliothèques si absent
```
