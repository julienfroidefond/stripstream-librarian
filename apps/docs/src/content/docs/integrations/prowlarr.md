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
- **Date de détection** (`detected_at`) : chaque release conserve la date de sa première découverte, même après un refresh

## Page Téléchargements

Tri disponible :

| Tri | Description |
|-----|-------------|
| **Récent** (défaut) | Par date de détection la plus récente parmi les releases |
| Seeders | Par nombre de seeders du meilleur release |
| Manquants | Par nombre de volumes manquants |
| Nom | Par nom de série |

La date relative de détection est affichée sur chaque série (ex : "il y a 2h").
