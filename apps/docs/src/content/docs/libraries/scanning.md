---
title: Scan & Indexation
description: Comment Stripstream indexe vos fichiers
---

## Comment fonctionne l'indexation

Quand vous lancez un scan, Stripstream travaille en deux étapes successives :

**Étape 1 — Découverte rapide** : Stripstream parcourt vos dossiers et enregistre immédiatement tous les nouveaux fichiers trouvés. Cette phase est très rapide car elle ne lit pas le contenu des archives — elle se base uniquement sur les noms de fichiers et la structure de dossiers. Vos livres apparaissent dans l'interface en quelques secondes.

**Étape 2 — Analyse approfondie** : Une fois la découverte terminée, Stripstream ouvre chaque archive pour compter les pages et extraire la première image afin de générer une miniature. Cette phase est plus longue mais s'exécute en arrière-plan sans bloquer l'interface.

:::note
Si un livre n'affiche pas encore de miniature ou de nombre de pages, c'est normal — l'étape 2 est peut-être encore en cours.
:::

---

## Types de scan

### Mise à jour (scan incrémental)

Le scan du quotidien. Stripstream se souvient de quels dossiers ont changé depuis la dernière fois et ne revisite que ceux-là. C'est le plus rapide.

**Quand l'utiliser :** usage courant, après avoir ajouté ou supprimé quelques fichiers.

---

### Rescan complet

Visite **tous** les dossiers (même ceux inchangés), mais conserve les livres déjà enregistrés et leurs métadonnées. Utile si l'indexer a raté des changements ou si vous venez de modifier la configuration.

**Quand l'utiliser :** si des fichiers semblent manquants alors qu'ils sont bien présents sur le disque.

---

### Reconstruction complète

**Efface d'abord tous les livres de la bibliothèque**, puis repart de zéro. Tout est recréé comme si la bibliothèque était scannée pour la première fois. Plus lent, mais garantit un état propre.

**Quand l'utiliser :** après une réorganisation majeure de l'arborescence, ou si la base de données semble incohérente.

:::caution
Les métadonnées éditées manuellement (descriptions, notes, liens metadata) **ne sont pas effacées** par une reconstruction complète — seuls les livres et leurs fichiers associés sont recréés.
:::

---

## Comment les séries sont détectées

La série d'un livre est déterminée par le **répertoire parent immédiat** du fichier. Si ce parent est un sous-dossier spécial (HS, Specials, Bonus, Extras, Intégrales, INT), le scanner remonte d'un cran pour trouver le nom de la série.

Exemple : `Shonen/Dragon Ball/T01.cbz` → série = "Dragon Ball"

---

## Détection du numéro de volume

Stripstream reconnaît plusieurs formats de nommage pour extraire le numéro de tome :

| Exemple de fichier | Volume détecté |
|--------------------|---------------|
| `Dragon Ball Tome 01.cbz` | Tome 1 |
| `Naruto T42.cbz` | Tome 42 |
| `One Piece Vol.12.pdf` | Volume 12 |
| `Bleach #5.cbz` | Numéro 5 |
| `Astérix -01.cbz` | Tome 1 |

---

## Types de volumes

Stripstream distingue quatre types de volumes :

| Type | Description |
|------|-------------|
| **Régulier** | Tome standard numéroté |
| **Hors-série** | Édition spéciale, bonus (détecté via HS, Hors-Série, Spécial… dans le nom) |
| **Intégrale** | Omnibus ou intégrale (détecté via INT, Intégrale… dans le nom) |
| **One-shot** | Livre autonome (fichier dans un dossier `Oneshots` à la racine) |

:::important
Les volumes **réguliers** participent au comptage des tomes et à la détection des volumes manquants. Les hors-séries et oneshots sont traités à part. Une **intégrale** marque la série comme complète pour le calcul des manquants.
:::

### Dossier Oneshots

Un dossier nommé `Oneshots` (ou variantes) placé **directement à la racine d'une bibliothèque** est traité spécialement : chaque fichier qu'il contient devient sa propre série avec un seul livre.

```
Ma Bibliothèque/
├── Oneshots/
│   ├── Blacksad.cbz       → série "Blacksad", 1 livre
│   └── Persepolis.cbz     → série "Persepolis", 1 livre
└── Dragon Ball/
    └── T01.cbz            → série "Dragon Ball", tome 1
```

Un dossier `Oneshots` imbriqué plus profondément n'est **pas** traité comme dossier oneshot.

Voir aussi la [gestion des séries](/series/management) pour le filtre par type de volume.

:::note[Détails techniques]
**Phase 1 — Discovery** : parcours WalkDir, empreinte `SHA256(taille + mtime + nom)` pour détecter les changements sans relire les archives, insertion avec `page_count = NULL`, skip des répertoires inchangés via la table `directory_mtimes`.

**Phase 2 — Analysis** : traite les livres où `page_count IS NULL`, ouvre les archives, génère des miniatures WebP, concurrence bornée par Semaphore.

**Patterns de volume reconnus** (par ordre de priorité) : `Tome ##`, `Tome.##`, `T##`, `Vol.##`, `Volume ##`, `###`, `-## `, `Tome_##`.

**Sous-dossiers spéciaux reconnus** (insensible à la casse) : `HS`, `Hors-Série`, `Hors Serie`, `Spécial`, `Specials`, `Spéciaux`, `Bonus`, `Extras`, `Extra`, `Intégrale`, `Intégrales`, `INT`.

**Noms de dossier Oneshots reconnus** (insensible à la casse, préfixe `_` ou `.` accepté) : `Oneshots`, `Oneshot`, `One-Shots`, `One-Shot`, `One Shots`, `One Shot`.

**Protection contre les faux volumes non montés** : le scanner ne conclut à un volume non monté que si les fichiers ont disparu **et** que leurs dossiers parents n'existent plus. Si l'arborescence est toujours présente (fichier simplement remplacé, renommé ou changé de format), le scan met à jour l'index normalement — ce qui évite les doublons lors d'un re-téléchargement.
:::
