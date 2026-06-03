---
title: Providers de métadonnées
description: Les 6 sources supportées pour enrichir vos séries
---

## Sources disponibles

Stripstream peut enrichir vos séries en allant chercher des informations sur 6 sources différentes :

| Source | Idéal pour | Remarques |
|--------|-----------|-----------|
| **Google Books** | Livres, romans, BD en français | Utilisé par défaut si aucun provider n'est configuré |
| **ComicVine** | Comics anglophones | Clé API requise (gratuite) |
| **Bédéthèque** | BD franco-belge | Scraping du site — top 3 résultats enrichis en détail |
| **AniList** | Manga, manhwa, manhua | Également utilisé pour la synchronisation de progression |
| **Open Library** | Livres généraux | Données ouvertes, catalogue mondial |
| **SensCritique** | BD et manga en français | Distingue les éditions d'une même série |

## Choisir le bon provider

Chaque bibliothèque peut avoir son propre provider principal et un provider de secours. Configurez-les dans les paramètres de la bibliothèque (icône ⚙️ → section **Métadonnées**).

:::tip
Si votre bibliothèque contient des mangas, utilisez **AniList** en priorité et **SensCritique** en secours. Pour la BD franco-belge, **Bédéthèque** donne généralement les résultats les plus précis.
:::

## Configurer ComicVine

ComicVine nécessite une clé API gratuite :

1. Créez un compte sur [comicvine.gamespot.com](https://comicvine.gamespot.com) et récupérez votre clé API dans les paramètres du compte
2. Dans Stripstream : **Settings → onglet Général → Providers → ComicVine API Key**

Sans clé configurée, les recherches ComicVine échouent silencieusement et tombent sur le provider de secours.

## Normalisation du statut de série

Les statuts retournés par les providers (`ongoing`, `ended`, `completed`…) ne sont pas toujours homogènes. Les **mappings de statut** permettent de normaliser ces valeurs vers vos propres labels.

Accès : **Settings → onglet Général → Status Mappings**.

:::note[Détails techniques]
**SensCritique** : utilise l'API GraphQL `apollo.senscritique.com`. Recherche via `searchAutocomplete` avec déduplication par franchise. Mode détaillé via `groupProducts` pour distinguer les éditions. Format `external_id` : `franchise:{id}:edition:{base64(name)}`, rétrocompatible avec `franchise:{id}`. Rate limiting : retry avec backoff exponentiel sur HTTP 429 (1s, 2s, 4s, 3 retries). Statut inféré depuis la dernière date de sortie : < 18 mois → `ongoing`, ≥ 18 mois → `ended`.

**Sources de description par provider** :

| Provider | Source de la description |
|----------|------------------------|
| SensCritique (batch) | Synopsis du tome au numéro le plus bas (autocomplete, 20 items) |
| SensCritique (détaillé) | Synopsis du tome au numéro le plus bas (groupProducts complet) |
| Google Books | Description du premier volume rencontré |
| AniList | Description au niveau série (GraphQL) |
| ComicVine | Description du volume (HTML nettoyé) |
| Bédéthèque | Meta description de la page série |
| Open Library | Description du premier volume rencontré |

Tous les providers stockent la `description` dans `metadata_json` pour garantir sa persistance lors du cycle match → approve → sync.
:::
