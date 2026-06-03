---
title: Formats supportés
description: Formats de fichiers gérés par Stripstream
---

## Formats pris en charge

Stripstream gère nativement quatre formats :

| Format | Extension | Description |
|--------|-----------|-------------|
| **CBZ** | `.cbz` | Archive ZIP contenant des images — le format le plus courant pour les BD et comics |
| **CBR** | `.cbr` | Archive RAR contenant des images — compatible mais peut être converti en CBZ |
| **PDF** | `.pdf` | Document PDF |
| **EPUB** | `.epub` | Livre numérique (ebooks, light novels) |

## Résilience face aux archives endommagées

Stripstream essaie de gérer les archives endommagées ou atypiques sans bloquer le reste de l'indexation :

- **CBZ endommagé** : utilise un lecteur alternatif si le répertoire central est corrompu
- **CBR** : extraction via `unar` avec fallback vers une lecture CBZ si nécessaire
- **PDF** : `pdfinfo` pour le comptage de pages, `pdftoppm` pour le rendu des pages
- **EPUB** : extraction ZIP standard

Si trop d'erreurs surviennent en rafale lors d'un scan, celui-ci s'interrompt automatiquement pour éviter de surcharger le système.

:::note[Détails techniques]
La détection du format utilise l'extension du fichier et les magic bytes (signature binaire). La protection contre les erreurs en rafale détecte un épuisement de descripteurs de fichiers (FD exhaustion) via un compteur d'erreurs I/O consécutives.
:::
