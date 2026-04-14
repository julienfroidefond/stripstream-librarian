---
title: Sources de découverte
description: Parcourir et découvrir de nouvelles séries
---

## Onglets disponibles

| Onglet | Source | Tri | Filtre période |
|--------|--------|-----|----------------|
| Nouveautés BD | SensCritique `productsByRelease` | Popularité | Mois / Année |
| Nouveautés Manga | SensCritique `productsByRelease` | Popularité | Mois / Année |
| Meilleures BD | SensCritique `productsByRelease` | Note | Mois / Année |
| Meilleurs Manga | SensCritique `productsByRelease` | Note | Mois / Année |
| Bédéthèque | Bédéthèque indispensables | Rang | — |
| SensCritique Top BD | SensCritique `top` | Rang | — |
| SensCritique Top Manga | SensCritique `poll` | Rang | — |
| AniList | AniList trending manga | Popularité | — |
| Prowlarr | Prowlarr releases agrégées | Seeders | — |

## Cache

| Type de liste | Durée du cache |
|---------------|---------------|
| Top / poll | Infini (invalidation manuelle via bouton refresh) |
| Trending / best | 24 heures |
| Prowlarr | 7 jours |

- Les séries déjà possédées sont filtrées côté serveur
- Les suggestions masquées sont filtrées (hide/unhide par utilisateur)

## Masquer des suggestions

- Masquez les séries indésirables des résultats de découverte
- Panel dédié pour voir et réafficher les séries masquées
- Persisté par provider + external_id, survit aux sessions
