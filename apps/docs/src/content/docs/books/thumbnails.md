---
title: Miniatures
description: Génération et gestion des miniatures
---

## Génération

Les miniatures sont générées à partir de la première page de chaque archive par le service **indexer** (phase 2 — analyse).

| Paramètre | Défaut |
|-----------|--------|
| Format de sortie | WebP |
| Dimensions | 300×400 |
| Qualité | Configurable |

:::note
L'API ne génère pas les miniatures — elle crée uniquement les jobs en base. C'est l'indexer qui effectue le rendu.
:::

## Opérations en masse

| Opération | Description |
|-----------|-------------|
| **Rebuild manquantes** | Génère les miniatures absentes uniquement |
| **Régénérer tout** | Supprime et recrée toutes les miniatures |

## Rendu de pages

L'API permet de rendre n'importe quelle page d'une archive :

| Paramètre | Valeurs |
|-----------|---------|
| Format de sortie | Original, JPEG, PNG, WebP |
| Qualité | 1–100 |
| Largeur max | 1–2160 px |
| Filtre de rééchantillonnage | lanczos3, nearest, triangle/bilinear |
| Limite de concurrence | 8 (par défaut, via semaphore) |

## Cache

Deux niveaux de cache pour les pages rendues :

| Niveau | Détails |
|--------|---------|
| **Mémoire LRU** | 512 entrées |
| **Disque** | Clé SHA256, structure à deux niveaux |

Clé de cache = `hash(chemin + page + format + qualité + largeur)`
