---
title: Métadonnées des livres
description: Informations disponibles sur chaque livre
---

![Page détail d'un livre — métadonnées, statut de lecture, et aperçu des pages](/screenshots/book-detail.png)

## Informations disponibles sur un livre

Chaque livre dans Stripstream dispose des informations suivantes :

**Extraites automatiquement lors de l'indexation** :
- Titre (depuis le nom de fichier)
- Série (depuis la structure de dossiers)
- Numéro de volume (depuis le nom de fichier)
- Type de volume (régulier, hors-série, intégrale — détection automatique)
- Nombre de pages (lors de la phase d'analyse)

**Enrichies par les providers de métadonnées** :
- Résumé du tome
- ISBN
- Date de publication
- Langue
- Auteurs

Tous ces champs peuvent être modifiés manuellement. Seuls les champs enrichis par les providers (résumé, ISBN, date de publication, langue, auteurs) peuvent être verrouillés : un verrou sur le titre, la série ou le numéro de volume n'a aucun effet, car la synchronisation ne modifie jamais ces champs.

## Verrouillage de champs

Chaque champ enrichi par les providers peut être verrouillé pour empêcher une synchronisation de métadonnées de l'écraser : résumé, ISBN, date de publication, langue et auteurs. Un champ verrouillé n'est jamais modifié automatiquement, quelle que soit la source.

:::note[Détails techniques]
**Champs extraits automatiquement** : `title`, `series` (répertoire parent), `volume` (parsing du nom), `volume_type` (regular/hs/integral/oneshot), `page_count` (phase 2), `kind` (ebook, comic, bd).

**Règles de mise à jour par les providers** :

| Champ | Règle |
|-------|-------|
| `summary` | Remplace si non-vide (`COALESCE(NULLIF(new, ''), existing)`) |
| `isbn` | Remplace si non-vide |
| `publish_date` | Remplace si non-vide |
| `language` | Remplace si non-vide |
| `authors` | Remplace si le nouveau tableau est non-vide |

La colonne `author` (premier auteur, utilisée pour l'affichage et le tri) est dérivée de `authors` et suit le même verrou.

Tous ces champs respectent le verrouillage via `locked_fields` (booléens stricts). Les clés inconnues ou non booléennes sont ignorées à l'écriture et purgées des lignes existantes par migration.
:::
