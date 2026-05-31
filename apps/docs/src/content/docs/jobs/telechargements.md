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

Au lieu de faire **N requêtes Prowlarr** (une par série), le job `prowlarr_rss` effectue **une seule requête** avec une query vide pour récupérer les releases récentes (RSS-style), puis les rapproche en mémoire de toutes les séries de la bibliothèque ayant des volumes manquants.

| | `download_detection` | `prowlarr_rss` |
|--|---------------------|----------------|
| Requêtes Prowlarr | 1 par série | 1 au total |
| Correspondance | Prowlarr filtre par nom | Matching local sur le titre |
| Couverture | Toute l'histoire de l'indexer | Releases récentes uniquement |
| Fréquence conseillée | Horaire / quotidien | Toutes les 30 min |

### Prérequis

Identiques à `download_detection` : Prowlarr configuré, séries avec un **lien metadata approuvé** et des volumes manquants.

### Règles métier

**Fetch des releases**

Une requête unique est envoyée à Prowlarr (`/api/v1/search?query=&type=search`) avec les catégories configurées. Prowlarr retourne les releases récentes de tous ses indexers.

**Matching titre / série**

Pour chaque série avec des volumes manquants, le titre de chaque release est normalisé puis comparé au nom de la série :

- Points et underscores → espaces
- Accents courants supprimés (`é→e`, `à→a`, etc.)
- Comparaison insensible à la casse

Si le titre normalisé **contient** le nom normalisé de la série, `match_title_volumes` est appliqué pour identifier les volumes couverts.

**Résultats et fusion**

Identiques à `download_detection` : upsert dans `available_downloads` via `merge_releases()`, `detected_at` préservé pour les releases déjà connues.

### Résultats par série

| `event_type` | Signification |
|-------------|---------------|
| `downloads_found` | Des releases correspondantes ont été trouvées dans le flux RSS |
| `downloads_not_found` | Aucune release ne correspond à cette série dans le flux courant |

### Rapport de job

| Champ | Signification |
|-------|--------------|
| `total_series` | Séries avec volumes manquants vérifiées |
| `found` | Séries pour lesquelles des releases ont été trouvées |
| `new_releases` | Nouvelles releases détectées lors de ce run |
| `rss_releases_fetched` | Nombre total de releases récupérées depuis Prowlarr |

### Ordonnancement automatique

Le job est déclenché automatiquement toutes les **30 minutes** par l'indexer pour chaque bibliothèque, à condition que :
- Prowlarr soit configuré
- Aucun job `prowlarr_rss` ne soit déjà actif pour cette bibliothèque
- Le dernier job `prowlarr_rss` terminé date de plus de 30 minutes

Aucun paramètre bibliothèque n'est requis (intervalle fixe, non configurable).

### API

```
POST /prowlarr-rss/start
{ "library_id": "uuid" }   // optionnel — toutes les bibliothèques si absent
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
