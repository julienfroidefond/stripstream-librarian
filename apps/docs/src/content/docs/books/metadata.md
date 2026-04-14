---
title: Métadonnées des livres
description: Champs de métadonnées extraits et gérés
---

## Champs extraits automatiquement

| Champ | Source |
|-------|--------|
| **Titre** | Nom de fichier ou métadonnées externes |
| **Série** | Structure de répertoires (parent immédiat) |
| **Volume** | Extraction depuis le nom de fichier |
| **Type de volume** | Détection automatique (regular, hs, integral) |
| **Auteur(s)** | Métadonnées externes ou édition manuelle |
| **Nombre de pages** | Analyse de l'archive (phase 2) |
| **Langue** | Métadonnées externes |
| **Kind** | ebook, comic, bd |

## Champs enrichis par les providers

Lors de la synchronisation des métadonnées, les champs suivants sont mis à jour sur chaque livre :

| Champ | Règle de mise à jour |
|-------|---------------------|
| `summary` | Remplace si non-vide (`COALESCE(NULLIF(new, ''), existing)`) |
| `isbn` | Remplace si non-vide |
| `publish_date` | Remplace si non-vide |
| `language` | Remplace si non-vide |
| `authors` | Remplace si le nouveau tableau est non-vide |

:::tip
Tous ces champs respectent le **verrouillage** : si un champ est verrouillé, la synchronisation le passe.
:::
