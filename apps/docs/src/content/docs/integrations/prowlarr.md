---
title: Prowlarr
description: Recherche d'indexeurs et détection de téléchargements
---

## Recherche

Recherchez Prowlarr pour les volumes manquants d'une série :

- Matching de patterns de volumes dans les titres (supporte `T##`, `Tome ##`, `Vol ##`, ranges `1-10`, intégrales)
- Résultats : titre, taille, seeders/leechers, URL, volumes manquants matchés
- Volume 0 (T0) exclu de la recherche

## Détection automatique

Job `download_detection` — scan automatique de toutes les séries avec volumes manquants :

- Stocke les releases disponibles dans `available_downloads`
- **`prowlarr_no_results`** (error) : Prowlarr a retourné 0 résultats bruts (problème indexeur)
- **`downloads_not_found`** (info) : résultats trouvés mais aucun ne correspond aux volumes manquants

## Releases

- **Indicateur d'échec** : badge sur les releases qui ont eu des erreurs de téléchargement précédentes
- **Blacklist** : masquez définitivement les releases indésirables (filtrées lors de la prochaine détection)
- Panel de blacklist avec possibilité de réafficher
