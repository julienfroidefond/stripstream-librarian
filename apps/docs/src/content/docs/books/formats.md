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

### PDF : page_count et vignettes

Le comptage de pages est **indépendant** du rendu de la première page. Si le rendu de la
première page échoue (page corrompue, délai dépassé), le livre est tout de même indexé avec
son nombre de pages exact ; seule la vignette reste absente (un job de reconstruction de
vignettes peut la retenter plus tard). Cas particuliers :

- **PDF chiffré avec mot de passe utilisateur** : ne peut pas être ouvert — l'erreur
  `PDF is password-protected` est enregistrée sur le fichier (aucun mot de passe n'est stocké).
- **PDF chiffré « owner password » uniquement** (permissions restreintes, ouverture sans mot
  de passe) : ouvert normalement.
- **Fichier `.pdf` corrompu ou qui n'est pas un PDF** : erreur explicite
  `PDF cannot be loaded (corrupted or not a PDF)`.

Si trop d'erreurs surviennent en rafale lors d'un scan, celui-ci s'interrompt automatiquement pour éviter de surcharger le système.

:::note[Détails techniques]
La détection du format se fait **uniquement par extension** de fichier. Les magic bytes ne sont utilisés qu'en interne pour choisir le bon lecteur (ZIP vs RAR) sur les archives atypiques. La protection contre les erreurs en rafale détecte un épuisement de descripteurs de fichiers (FD exhaustion) via un compteur d'erreurs I/O consécutives.
:::
