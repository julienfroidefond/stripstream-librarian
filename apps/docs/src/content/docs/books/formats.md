---
title: Formats supportés
description: Formats de fichiers gérés par Stripstream
---

## Formats

| Format | Extension | Description |
|--------|-----------|-------------|
| **CBZ** | `.cbz` | Archive ZIP contenant des images (BD/comics) |
| **CBR** | `.cbr` | Archive RAR contenant des images (BD/comics) |
| **PDF** | `.pdf` | Document PDF |
| **EPUB** | `.epub` | Livre numérique (basé ZIP) |

La détection du format utilise l'extension du fichier et les magic bytes.

## Résilience des archives

Stripstream gère les archives endommagées ou atypiques :

| Format | Fallback |
|--------|----------|
| CBZ | Lecteur streaming si le répertoire central est corrompu |
| CBR | Extraction RAR via `unar`, fallback vers parsing CBZ |
| PDF | `pdfinfo` pour le comptage, `pdftoppm` pour le rendu |
| EPUB | Extraction ZIP standard |

### Détection d'épuisement de FD

Si trop d'erreurs I/O consécutives surviennent, le scan s'interrompt pour éviter l'épuisement des descripteurs de fichiers.
