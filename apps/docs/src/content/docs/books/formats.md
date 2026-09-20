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
- **CBR** : lecture RAR **in-process** via le crate `unrar` (aucun binaire externe), avec repli sur une lecture ZIP si nécessaire
- **PDF** : comptage de pages et rendu via **pdfium** (bibliothèque `libpdfium`) — pas de `pdfinfo`/`pdftoppm`
- **EPUB** : extraction ZIP standard

Si trop d'erreurs surviennent en rafale lors d'un scan, celui-ci s'interrompt automatiquement pour éviter de surcharger le système.

:::note[Détails techniques]
La détection du format se fait **uniquement par extension** de fichier. Les magic bytes ne sont utilisés qu'en interne pour choisir le bon lecteur (ZIP vs RAR) sur les archives atypiques. La protection contre les erreurs en rafale détecte un épuisement de descripteurs de fichiers (FD exhaustion) via un compteur d'erreurs I/O consécutives.
:::
