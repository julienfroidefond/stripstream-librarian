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

**Gérées manuellement** :
- Auteurs (édition directe)
- Tous les champs ci-dessus peuvent être modifiés et verrouillés

## Verrouillage de champs

Chaque champ peut être verrouillé pour empêcher une synchronisation de métadonnées de l'écraser. Un champ verrouillé n'est jamais modifié automatiquement, quelle que soit la source.

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

Tous ces champs respectent le verrouillage via `locked_fields`.
:::
