---
title: Jobs d'indexation
description: Scan de bibliothèque — rebuild, rescan, full_rebuild
---

Les jobs d'indexation scannent le système de fichiers pour maintenir la base de données des livres à jour. Ils fonctionnent en **deux phases** et sont exécutés par le service **Indexer**.

---

## Pipeline en deux phases

### Phase 1 — Discovery (scanner)

- Parcourt les dossiers via WalkDir
- Calcule un **fingerprint** par fichier : `SHA256(taille + mtime + nom de fichier)` — détecte les changements sans ouvrir les archives
- Insère les nouveaux livres avec `page_count = NULL` pour les rendre visibles immédiatement
- Détecte les fichiers supprimés du disque et les retire de la DB
- **Protection contre suppression massive** : si tous les fichiers existants semblent avoir disparu (ex. disque démonté), la suppression est annulée pour éviter une perte de données

### Phase 2 — Analysis (analyzer)

Traite les livres avec `page_count IS NULL` :

- Ouvre l'archive (CBZ, CBR, PDF, EPUB)
- Extrait le nombre de pages
- Lit la première image → génère une miniature WebP
- Concurrence bornée par un Semaphore pour éviter de saturer le disque

:::note
Le statut du job passe à `extracting_pages` puis `generating_thumbnails` lors de la phase 2.
:::

---

## `rebuild` — Scan incrémental

Le scan du quotidien. Visite uniquement les dossiers dont la date de modification a changé depuis le dernier scan.

### Règles métier

- Charge la table `directory_mtimes` : si un dossier n'a pas changé depuis le dernier scan, son contenu est entièrement sauté (zéro I/O)
- Met à jour `directory_mtimes` après chaque dossier traité
- Les répertoires `HS/Specials/Bonus/Intégrales` remontent d'un cran pour nommer la série parente
- Re-matche les `external_book_metadata` sans `book_id` par numéro de volume après le scan (corrige les métadonnées de livres nouvellement indexés)
- Propage le `volume_type` lors des updates (livres dont le fingerprint n'a pas changé)

### Déclenchement automatique

Déclenché par l'ordonnanceur si `monitor_enabled = true` et `next_scan_at <= NOW()`. Le mode `scan_mode` détermine la fréquence.

### API

```
POST /index/rebuild
{ "library_id": "uuid" }   // optionnel — toutes les bibliothèques si absent
```

---

## `rescan` — Rescan profond

Vide le cache des dates de modification pour forcer le re-parcours de tous les dossiers, sans supprimer les données existantes.

### Règles métier

- Vide la table `directory_mtimes` pour la bibliothèque avant de commencer
- Toutes les données (livres, séries, métadonnées, statuts de lecture) sont **conservées**
- Le comptage du total de fichiers se fait sur le filesystem (plus lent qu'un rebuild normal qui lit la DB)

### Cas d'usage

- Après l'ajout d'un format supporté (ex. EPUB) pour le découvrir sans tout reconstruire
- Pour corriger des `volume_type` mal classés (HS déclarés comme regular) sans perdre les métadonnées

### API

```
POST /index/rebuild
{ "library_id": "uuid", "rescan": true }
```

---

## `full_rebuild` — Reconstruction complète ⚠️

Supprime toute la base de données de la bibliothèque et rescanne depuis zéro.

### Règles métier

- **Supprime tous les livres et fichiers** (`books`, `book_files`) de la bibliothèque avant de commencer
- Vide `directory_mtimes`
- Le comptage se fait sur le filesystem (DB vide)
- Après la phase 2, **nettoie les miniatures orphelines** : les fichiers WebP dont l'UUID n'est plus référencé en DB (générés pour d'anciens livres) sont supprimés du disque

:::caution
Toutes les **métadonnées approuvées**, **statuts de lecture** et **liens AniList** associés aux séries sont perdus car les séries sont supprimées et recréées avec de nouveaux UUIDs. À utiliser uniquement si la bibliothèque est corrompue ou si vous souhaitez repartir de zéro.
:::

### Déclenchement automatique

Possible si `scan_mode = "full"` dans les paramètres de la bibliothèque.

### API

```
POST /index/rebuild
{ "library_id": "uuid", "full": true }
```

---

## `scan` — Scan par watcher

Job créé automatiquement par le watcher de système de fichiers (inotify/kqueue) quand un changement est détecté dans un dossier surveillé.

### Règles métier

- Se comporte comme un `rebuild` standard
- Exclusif : ne s'exécute pas si un autre job exclusif est déjà actif pour cette bibliothèque
- Priorité 2 (même que rebuild)

---

## Événements de job

| `event_type` | Niveau | Signification |
|-------------|--------|---------------|
| `error` | `error` | Fichier non lisible ou archive corrompue |
