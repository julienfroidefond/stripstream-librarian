---
title: Sources de découverte
description: Parcourir et découvrir de nouvelles séries
---

## Onglets disponibles

| Onglet | Source | Tri | Filtre période |
|--------|--------|-----|----------------|
| Nouveautés BD | SensCritique `productsByRelease` | Popularité | Mois / Année / Tous les temps |
| Nouveautés Manga | SensCritique `productsByRelease` | Popularité | Mois / Année / Tous les temps |
| Meilleures BD | SensCritique `productsByRelease` | Note | Mois / Année / Tous les temps |
| Meilleurs Manga | SensCritique `productsByRelease` | Note | Mois / Année / Tous les temps |
| Bédéthèque | Bédéthèque indispensables | Rang | — |
| SensCritique Top BD | SensCritique `top` | Rang | — |
| SensCritique Top Manga | SensCritique `poll` | Rang | — |
| AniList | AniList trending manga | Popularité | — |
| Prowlarr | Prowlarr releases individuelles | Seeders + récents | Catégorie |

## Cache

| Type de liste | Durée du cache |
|---------------|---------------|
| Top / poll | Infini (invalidation manuelle via bouton refresh) |
| Trending / best | 24 heures |
| Prowlarr | 7 jours |

- Les séries déjà possédées sont filtrées côté serveur (annotées "Déjà possédée" dans l'onglet Prowlarr)
- Les suggestions masquées sont filtrées (hide/unhide par utilisateur)
- L'onglet Prowlarr affiche chaque **release individuellement** (titre, catégorie, indexer, seeders) — le filtre catégorie déclenche une requête Prowlarr ciblée

## Recommandations personnalisées

Un onglet **Recommandations** propose des séries de votre bibliothèque non encore commencées, basées sur vos dernières lectures. Voir [Séries liées & Recommandations](/series/related/) pour le détail du scoring.

## Masquer des suggestions

- Masquez les séries indésirables des résultats de découverte
- Panel dédié pour voir et réafficher les séries masquées
- Persisté par provider + external_id, survit aux sessions
