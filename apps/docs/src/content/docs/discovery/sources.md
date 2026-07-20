---
title: Sources de découverte
description: Parcourir et découvrir de nouvelles séries
---

La page **Découverte** vous permet de parcourir des listes de séries populaires ou récentes depuis plusieurs sources, même sans les avoir dans votre bibliothèque.

## Onglets disponibles

| Onglet | Ce qu'il propose |
|--------|-----------------|
| **Nouveautés BD** | BD récentes triées par popularité (mois / année / tous les temps) |
| **Nouveautés Manga** | Mangas récents triés par popularité |
| **Meilleures BD** | BD les mieux notées |
| **Meilleurs Manga** | Mangas les mieux notés |
| **Bédéthèque** | Les incontournables de la BD franco-belge selon Bédéthèque |
| **SensCritique Top BD** | Classement BD de SensCritique |
| **SensCritique Top Manga** | Classement manga de SensCritique |
| **AniList** | Mangas en tendance sur AniList |
| **Prowlarr** | Releases récentes disponibles sur vos indexeurs |
| **Recommandations** | Séries de votre bibliothèque à découvrir selon vos lectures |

Les cartes des listes **Nouveautés** et **Meilleures** affichent la note moyenne SensCritique lorsqu'elle est disponible.

## Ajouter à la bibliothèque

Sur chaque carte de découverte, un bouton **"+"** permet d'ajouter la série à votre bibliothèque en un clic. Voir [Ajouter à la bibliothèque](/discovery/add-to-library/) pour le détail.

Les séries que vous possédez déjà sont signalées comme telles.

## Masquer des suggestions

Vous pouvez masquer les séries qui ne vous intéressent pas pour qu'elles n'apparaissent plus dans les résultats. Un panel dédié vous permet de retrouver et réafficher les séries masquées.

## Recommandations personnalisées

L'onglet **Recommandations** propose des séries de votre bibliothèque que vous n'avez pas encore commencées, en fonction de vos dernières lectures. Voir [Séries liées & Recommandations](/series/related/) pour le détail.

:::note[Détails techniques]
**Sources** :
- Nouveautés / Meilleures : SensCritique `productsByRelease` (popularité ou note)
- Bédéthèque : scraping des indispensables
- Tops SensCritique : requêtes `top` et `poll`
- AniList : trending manga
- Prowlarr : releases individuelles (titre, catégorie, indexer, seeders)

**Durées de cache** :
| Type de liste | Durée |
|---------------|-------|
| Top / poll | Infini (invalidation manuelle via bouton refresh) |
| Trending / best | 24 heures |
| Prowlarr | 7 jours |

**Filtrage** : les séries déjà possédées sont filtrées côté serveur. Les suggestions masquées sont filtrées par `(provider, external_id)` et persistent entre les sessions.

L'onglet Prowlarr affiche chaque release individuellement. Le filtre catégorie déclenche une requête Prowlarr ciblée avec `?indexer=X`.
:::
