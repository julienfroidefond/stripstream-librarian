---
title: Tâches d'indexation
description: Scan de bibliothèque — mise à jour, rescan, reconstruction complète
---

Les tâches d'indexation maintiennent votre bibliothèque à jour en scannant le système de fichiers. Elles fonctionnent en deux phases (découverte rapide + analyse approfondie) et sont exécutées par le service Indexer.

---

## Mise à jour — Scan incrémental

Le scan du quotidien. Stripstream se souvient de quels dossiers ont changé et ne revisite que ceux-là — le plus rapide.

**Quand l'utiliser :** usage courant, après avoir ajouté ou supprimé des fichiers.

**Déclenchement automatique :** configurez la fréquence dans les paramètres de la bibliothèque → section Indexation.

---

## Rescan — Parcours complet

Visite **tous** les dossiers de votre bibliothèque (même les inchangés), mais conserve tous vos livres, métadonnées et statuts de lecture. Plus lent qu'un scan incrémental, mais ne perd aucune donnée.

**Quand l'utiliser :** si des fichiers semblent manquants alors qu'ils sont bien sur le disque, ou après avoir ajouté un nouveau format (ex. EPUB) que vous souhaitez découvrir sans tout reconstruire.

---

## Reconstruction complète — Repartir de zéro

:::caution
Supprime **tous les livres** de la bibliothèque en base de données avant de rescanner. **Toutes les métadonnées approuvées, statuts de lecture et liens AniList sont perdus** car les séries sont supprimées et recréées avec de nouveaux identifiants. À utiliser uniquement si la bibliothèque est corrompue ou si vous souhaitez repartir de zéro.
:::

Après la reconstruction, les miniatures orphelines (liées à d'anciens livres) sont supprimées du disque.

**Quand l'utiliser :** après une réorganisation majeure de l'arborescence, ou si la base de données semble incohérente.

---

## Scan par surveillance (watcher)

Quand la surveillance en temps réel est activée pour une bibliothèque, les modifications détectées directement sur le disque (ajout de fichier, suppression effectuée hors de Stripstream) déclenchent automatiquement une mise à jour incrémentale de la bibliothèque.

Les suppressions effectuées depuis Stripstream (livre ou série) sont appliquées directement en base et ne déclenchent **aucun scan supplémentaire**. Supprimer un livre met immédiatement à jour sa série, et si c'était le dernier livre, la série est archivée et retirée sans attendre.

---

## Protection contre les suppressions massives

Si un scan détecte que la quasi-totalité de vos fichiers ont "disparu" (par exemple si le disque est démonté accidentellement), la suppression est annulée automatiquement pour éviter une perte de données.

:::note[Détails techniques]
**Phase 1 — Discovery** : WalkDir, fingerprint `SHA256(taille + mtime + nom)`, insertion avec `page_count = NULL`, skip des répertoires inchangés via `directory_mtimes`. Détecte les fichiers supprimés et les retire de la DB.

**Phase 2 — Analysis** : traite les livres où `page_count IS NULL`, ouvre les archives, extrait le nombre de pages, génère une miniature WebP. Concurrence bornée par Semaphore.

**Réscan** : vide la table `directory_mtimes` avant de commencer (force le re-parcours de tous les dossiers). Toutes les données conservées.

**Reconstruction complète** : supprime `books` et `book_files`, vide `directory_mtimes`. Après la phase 2, nettoie les fichiers WebP orphelins dont l'UUID n'est plus en DB.

**API** :
```
POST /index/rebuild
{ "library_id": "uuid" }            // scan incrémental
{ "library_id": "uuid", "rescan": true }  // rescan
{ "library_id": "uuid", "full": true }    // reconstruction complète
```

**Événement de job** : `event_type = 'error'` (niveau `error`) pour les fichiers non lisibles ou archives corrompues.
:::
