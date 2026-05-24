---
title: Providers de métadonnées
description: Les 6 providers supportés pour enrichir vos séries
---

## Providers disponibles

| Provider | Focus | API | Description |
|----------|-------|-----|-------------|
| **Google Books** | Livres général | REST | Fallback par défaut |
| **ComicVine** | Comics | REST | Clé API requise |
| **Bedetheque** | BD franco-belge | Scraping HTML | Top 3 enrichis en détail |
| **AniList** | Manga/anime | GraphQL | + sync progression de lecture |
| **Open Library** | Livres général | REST | Données ouvertes |
| **SensCritique** | BD, manga, comics | GraphQL | Éditions, franchises |

## SensCritique — Détails

### Recherche
- `searchAutocomplete` avec déduplication par franchise
- Mode détaillé : breakdown par édition via `groupProducts`
- Chaque édition retournée comme un candidat séparé avec `total_volumes` précis

### Éditions
- Détection depuis les titres de produits (pattern : `"Titre - Édition, tome X"`)
- Format `external_id` : `franchise:{id}:edition:{base64(name)}`
- Rétrocompatible avec `franchise:{id}`

### Statut de série
Inféré depuis la dernière date de sortie :
- Publication < 18 mois → `ongoing`
- Publication ≥ 18 mois → `ended`

### Rate limiting
- Retry avec backoff exponentiel sur HTTP 429 (1s, 2s, 4s, 3 retries)
- Throttle 300ms entre requêtes dans les jobs de refresh

## Source de la description par provider

| Provider | Source de la description |
|----------|------------------------|
| SensCritique (batch) | Synopsis du tome au numéro le plus bas (autocomplete, 20 items) |
| SensCritique (détaillé) | Synopsis du tome au numéro le plus bas (groupProducts complet par édition) |
| Google Books | Description du premier volume rencontré pendant le groupement |
| AniList | Description au niveau série (GraphQL) |
| ComicVine | Description du volume (HTML nettoyé) |
| Bedetheque | Meta description de la page série (scraping, top 3 enrichis) |
| Open Library | Description du premier volume rencontré |

:::note
Tous les providers stockent la `description` dans `metadata_json` pour garantir sa persistance lors du cycle match → approve → sync.
:::

## Configuration

- Provider par défaut configurable **par bibliothèque** dans les paramètres de la bibliothèque (icône ⚙️ → section Métadonnées)
- Provider de fallback si le principal est indisponible

### Clé API ComicVine

ComicVine requiert une clé API gratuite :

1. Créez un compte sur [comicvine.gamespot.com](https://comicvine.gamespot.com) et récupérez votre clé API dans les paramètres du compte
2. Dans Stripstream : **Settings → onglet Général → Providers → ComicVine API Key**

Sans clé configurée, les recherches ComicVine échouent silencieusement et tombent sur le provider de fallback.
