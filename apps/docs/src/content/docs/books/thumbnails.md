---
title: Miniatures
description: Génération et gestion des miniatures
---

## Comment les miniatures sont générées

Stripstream génère automatiquement une miniature pour chaque livre en extrayant la première page de l'archive. Cette opération est effectuée en arrière-plan lors de la phase d'analyse (phase 2 du scan).

:::note
Les miniatures sont générées par le service **Indexer**, pas par l'API. Si une miniature est manquante juste après un scan, attendez quelques instants que la phase d'analyse se termine.
:::

## Regénérer les miniatures

Depuis la page **Tâches**, deux options sont disponibles :

| Opération | Description |
|-----------|-------------|
| **Générer les miniatures manquantes** | Génère uniquement les miniatures absentes — ne touche pas celles qui existent déjà |
| **Regénérer toutes les miniatures** | Supprime et recrée toutes les miniatures |

:::caution
La regénération complète supprime temporairement toutes les miniatures pendant l'exécution. Sur une grande bibliothèque, cela peut prendre plusieurs minutes.
:::

## Lecture de pages

En dehors des miniatures, Stripstream peut rendre n'importe quelle page d'une archive à la demande pour les lecteurs externes. Les pages rendues sont mises en cache pour éviter de rouvrir les archives à chaque requête.

:::note[Détails techniques]
**Format de sortie des miniatures** : WebP, 300×400 px.

**Rendu de pages à la demande** : formats JPEG, PNG, WebP ou original ; qualité 1–100 ; largeur max 1–2160 px ; filtres lanczos3, nearest, triangle/bilinear. Concurrence limitée à 8 via Semaphore.

**Cache des pages rendues** :
- Mémoire LRU : 512 entrées
- Disque : clé `SHA256(chemin + page + format + qualité + largeur)`, structure à deux niveaux
:::
