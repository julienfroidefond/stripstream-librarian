---
title: Job de détection de téléchargements
description: Détection automatique des releases disponibles via Prowlarr
---

Le job `download_detection` interroge Prowlarr pour trouver des releases correspondant aux volumes manquants de chaque série. Il est exécuté par le service **API** (job poller) et est **non-exclusif**.

---

## `download_detection`

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

## Ordonnancement automatique

Si `download_detection_mode != 'manual'` et que Prowlarr est configuré, l'ordonnanceur crée automatiquement un job selon l'intervalle défini dans les paramètres de la bibliothèque.

Condition supplémentaire : aucun job `download_detection` ne doit être déjà actif (`pending` ou `running`) pour cette bibliothèque.

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
