---
title: Providers de métadonnées
description: Les 7 sources supportées pour enrichir vos séries
---

## Sources disponibles

Stripstream peut enrichir vos séries en allant chercher des informations sur 7 sources différentes :

| Source | Idéal pour | Remarques |
|--------|-----------|-----------|
| **Google Books** | Livres, romans, BD en français | Utilisé par défaut si aucun provider n'est configuré |
| **ComicVine** | Comics anglophones | Clé API requise (gratuite) |
| **BDTheque** | BD franco-belge | Volumes, ISBN et métadonnées de série ; top 8 résultats enrichis en détail |
| **BDphile** | BD franco-belge | Éditions, EAN et métadonnées détaillées d’albums ; top 8 résultats enrichis en détail |
| **AniList** | Manga, manhwa, manhua | Également utilisé pour la synchronisation de progression |
| **Open Library** | Livres généraux | Données ouvertes, catalogue mondial |
| **SensCritique** | BD et manga en français | Distingue les éditions d'une même série |

:::caution
Le provider **Bédéthèque** (`bedetheque.com`) a été retiré : le site est désormais protégé par un challenge Cloudflare qui empêche le scraping. Les bibliothèques configurées dessus basculent automatiquement vers **BDTheque**, et les liens de métadonnées restants sont supprimés par la migration `0114`.
:::

## Choisir le bon provider

Chaque bibliothèque peut avoir son propre provider principal et un provider de secours. Configurez-les dans les paramètres de la bibliothèque (icône ⚙️ → section **Métadonnées**).

:::tip
Si votre bibliothèque contient des mangas, utilisez **AniList** en priorité et **SensCritique** en secours. Pour la BD franco-belge, utilisez **BDTheque** en priorité, avec **BDphile** en secours.
:::

## Providers BD franco-belge

### BDTheque

[BDTheque](https://www.bdtheque.com) couvre les séries de BD franco-belge.

- Recherche par titre, puis enrichissement des 8 meilleurs résultats avec le détail complet de la série.
- Métadonnées : scénariste(s), dessinateur(s), éditeur, année de début, statut, nombre de tomes, couverture et description.
- Les noms d'auteurs sont normalisés au format `Prénom Nom` (la source renvoie `Nom (Prénom)`).
- `external_id` : identifiant et slug de la série dans l'URL (ex. `207/asterix`).

### BDphile

[BDphile](https://www.bdphile.fr) couvre également la BD franco-belge, avec un focus sur les éditions.

- Recherche via l'endpoint AJAX du site, puis enrichissement des 8 meilleurs résultats.
- Métadonnées : auteurs, éditeurs, date de parution, résumé, couverture et nombre de tomes.
- Seuls les crédits **scénario** et **dessin** sont conservés côté auteurs.
- `external_id` : slug de la série (ex. `bd/124-asterix`).

### Matching par ISBN / EAN

BDTheque et BDphile exposent l'ISBN (BDTheque) ou l'EAN (BDphile) des albums. Ce matching est **additif** : il rattache un tome local au bon album par ISBN/EAN, mais ne remplace jamais un match provider existant.

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

Pour une édition SensCritique, les métadonnées de série sont celles du tome au numéro le plus bas. Les auteurs regroupent les champs `authors` et `pencillers` de ce tome ; la même règle est appliquée au matching initial et aux refresh.

**Sources de description par provider** :

| Provider | Source de la description |
|----------|------------------------|
| SensCritique (batch) | Synopsis du tome au numéro le plus bas (autocomplete, 20 items) |
| SensCritique (détaillé) | Synopsis du tome au numéro le plus bas (groupProducts complet) |
| Google Books | Description du premier volume rencontré |
| AniList | Description au niveau série (GraphQL) |
| ComicVine | Description du volume (HTML nettoyé) |
| BDTheque | Meta description de la page série |
| BDphile | Synopsis de la page série |
| Open Library | Description du premier volume rencontré |

Tous les providers stockent la `description` dans `metadata_json` pour garantir sa persistance lors du cycle match → approve → sync.
:::
