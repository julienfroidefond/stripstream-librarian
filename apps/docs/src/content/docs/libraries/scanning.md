---
title: Scan & Indexation
description: Pipeline d'indexation en deux phases (discovery + analysis)
---

## Pipeline en deux phases

### Phase 1 — Découverte (Discovery)

Scan rapide basé uniquement sur les noms de fichiers, **sans I/O sur les archives** :

- Parcours des répertoires avec `WalkDir`
- Extraction des métadonnées depuis le nom de fichier (titre, volume, série)
- Insertion des livres avec `page_count = NULL` pour visibilité immédiate
- Skip des répertoires inchangés via la table `directory_mtimes`

:::note
Un `page_count = NULL` est normal après la phase discovery — la phase analysis le remplit ensuite.
:::

### Phase 2 — Analyse (Analysis)

Traitement approfondi des archives :

- Ouverture des archives, extraction du nombre de pages
- Extraction de la première page pour les miniatures
- Génération des miniatures WebP
- Traitement des livres où `page_count IS NULL`

### Fingerprint

Chaque fichier est identifié par un fingerprint : `SHA256(taille + mtime + nom)`. Cela permet de détecter les changements sans relire les fichiers.

## Types de scan

### Scan incrémental (défaut)

Le plus rapide. L'indexer se souvient de la date de dernière modification de chaque dossier — il ne revisite que les dossiers qui ont changé depuis le dernier scan. Les livres déjà connus restent intacts dans la base.

**Quand l'utiliser :** usage courant, après avoir ajouté ou supprimé quelques fichiers.

---

### Rescan

Visite **tous** les dossiers (même ceux inchangés), mais conserve les livres déjà enregistrés. Utile si l'indexer a raté des changements ou si vous venez de modifier la configuration (nouveaux formats supportés, renommage de dossiers...).

**Quand l'utiliser :** si des fichiers semblent manquants alors qu'ils sont bien présents sur le disque.

---

### Scan complet

**Efface d'abord tous les livres de la bibliothèque**, puis repart de zéro. C'est l'option nucléaire : tout est recréé comme si la bibliothèque était scannée pour la première fois. Plus lent, mais garantit un état propre.

**Quand l'utiliser :** après une réorganisation majeure de l'arborescence, ou si la base de données semble incohérente.

:::caution
Les métadonnées éditées manuellement (descriptions, notes, liens metadata) **ne sont pas effacées** par un scan complet — seuls les livres et leurs fichiers associés sont recréés.
:::

## Détection des séries

La série est dérivée du **répertoire parent immédiat** du fichier. Si ce parent est un sous-dossier spécial (HS, Specials, Bonus, Extras, Intégrales, INT), le scanner remonte d'un cran.

Exemple : `Shonen/Dragon Ball/T01.cbz` → série = "Dragon Ball"

## Extraction du volume

Patterns supportés (par ordre de priorité) :

| Pattern | Exemple |
|---------|---------|
| `Tome ##`, `Tome.##` | `Dragon Ball Tome 01.cbz` |
| `T##` | `Naruto T42.cbz` |
| `Vol.##`, `Volume ##` | `One Piece Vol.12.pdf` |
| `###` (hash) | `Bleach #5.cbz` |
| `-## ` (tiret) | `Astérix -01.cbz` |
| `Tome_##` | `Berserk Tome_33.cbz` |

## Type de volume

| Type | Description | Détection |
|------|-------------|-----------|
| `regular` | Volume standard numéroté (défaut) | Par défaut |
| `hs` | Hors-série / édition spéciale | HS, Hors-Série, Spécial, Bonus |
| `integral` | Omnibus / intégrale | INT, INTHS, Intégrale |
| `oneshot` | Livre autonome | Dossier Oneshots à la racine de la bibliothèque (auto) ou manuel |

:::important
Seuls les volumes `regular` participent à la numérotation des tomes. Les HS, oneshot et intégrales sont exclus du comptage de manquants et du matching metadata.
:::

### Dossier Oneshots

Un dossier placé **directement à la racine d'une bibliothèque** dont le nom correspond à un pattern oneshot est traité automatiquement : chaque fichier devient sa propre série avec un seul livre de type `oneshot`.

Noms reconnus (insensible à la casse, préfixe `_` ou `.` accepté) : `Oneshots`, `Oneshot`, `One-Shots`, `One-Shot`, `One Shots`, `One Shot`.

```
Ma Bibliothèque/
├── Oneshots/
│   ├── Blacksad.cbz       → série "Blacksad", 1 livre, volume_type = oneshot
│   └── Persepolis.cbz     → série "Persepolis", 1 livre, volume_type = oneshot
└── Dragon Ball/
    └── T01.cbz            → série "Dragon Ball", volume_type = regular
```

- Le titre du livre = nom de fichier sans extension
- Aucun numéro de volume assigné
- Un dossier `Oneshots` imbriqué (2 niveaux ou plus) n'est **pas** traité comme dossier oneshot

Voir aussi la [gestion des séries](/series/management) pour le filtre par type de volume.
