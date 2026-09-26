---
title: Archives
description: Conserver les métadonnées et la progression de lecture des séries supprimées du disque
---

Quand une série disparaît de votre disque (fichiers supprimés, dossier déplacé, série retirée), Stripstream l'archive automatiquement au lieu de tout effacer. Les métadonnées et la progression de lecture de chaque utilisateur sont préservées.

---

## Ce qui déclenche une archivage

Une série est archivée dans deux situations :

- **Suppression manuelle** depuis le backoffice — le bouton *Supprimer* dans les actions d'une série archive avant de supprimer.
- **Scan** — quand le scanner détecte qu'une série n'a plus aucun livre sur le disque (et qu'elle n'a pas de métadonnées approuvées ni de volumes en wishlist), il l'archive automatiquement.

## Ce qui est conservé

| Élément | Conservé |
|---------|----------|
| Nom, description, auteurs, éditeur, genres, année, statut | ✓ |
| Couverture (URL) | ✓ (URL uniquement — le fichier est supprimé) |
| Métadonnées de chaque livre (titre, tome, format, ISBN…) | ✓ |
| Progression de lecture par utilisateur | ✓ |
| Fichiers physiques (CBZ, PDF…) | ✗ supprimés du disque |
| Miniatures | ✗ supprimées |

---

## Consulter les archives

Les archives sont accessibles via **Paramètres → Archives**.

La liste affiche toutes les séries archivées avec leurs informations principales. Si des utilisateurs ont une progression de lecture sur cette série, elle est visible directement sur chaque ligne (ex. `3/12 Juju`).

Cliquez sur une ligne pour la déplier et voir le détail des livres archivés avec :
- Numéro de volume, titre, auteur, nombre de pages, format, date de publication
- Progression de lecture par utilisateur sur chaque livre (Lu / En cours p.X / —)

---

## Restauration automatique

Quand une série est réajoutée à la bibliothèque — que ce soit via un scan ou via **Découverte → Ajouter à la bibliothèque** — Stripstream détecte automatiquement qu'une archive correspondante existe et restaure les données :

- La description, les auteurs, les éditeurs, les genres et les autres métadonnées sont réappliqués sur la nouvelle série (sans écraser les champs déjà renseignés).
- La progression de lecture de chaque utilisateur est restaurée sur les livres retrouvés, rattachée **par numéro de tome**.
- L'entrée dans les archives est supprimée une fois la restauration effectuée.

Le rapprochement de la série se fait sur le nom (insensible à la casse et aux accents) au sein de la même bibliothèque. La progression, elle, est rattachée aux livres par **numéro de tome** : même si le nom de la série a changé, la progression est restaurée sur le tome correspondant.

---

:::note[Détails techniques]
Les archives sont stockées dans quatre tables séparées : `archived_series`, `archived_books`, `archived_book_files`, `archived_book_reading_progress`.

L'archivage utilise `ON CONFLICT DO UPDATE` depuis la suppression manuelle (les données fraîches écrasent) et `ON CONFLICT DO NOTHING` depuis le scanner (le premier archivage prime).

La restauration utilise `COALESCE` / `CASE WHEN array_length() > 0` pour ne pas écraser les champs déjà renseignés sur la série active.
:::
