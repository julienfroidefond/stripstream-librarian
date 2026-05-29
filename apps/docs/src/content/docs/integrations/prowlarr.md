---
title: Prowlarr
description: Recherche d'indexeurs et détection de téléchargements
---

## Découverte (onglet Prowlarr)

L'onglet **Prowlarr** de la page Découverte agrège les releases disponibles par série. Le bouton "+" ouvre un modal d'ajout guidé — voir [Ajouter à la bibliothèque](/discovery/add-to-library/) pour le détail du flow.

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

## Stratégie de fetch deux passes

Pour la page Découverte (onglet Prowlarr), le fetch se fait en deux passes afin de combiner meilleures sources et sorties récentes :

1. **Passe 1** — plusieurs requêtes par mots-clés (genres BD/manga), triées par **seeders** — remonte les releases les plus distribuées
2. **Passe 2** — une requête vide triée par **publishDate décroissant** — remonte les sorties les plus récentes qui pourraient être absentes du top seeders

Les résultats des deux passes sont dédupliqués par GUID et mis en cache **7 jours**. La limite totale de résultats retournés est de **200**.

### Filtrage par indexer

Tous les indexers configurés dans Prowlarr apparaissent dans les boutons de filtre, même si leurs releases sont hors du top 200 global (ils sont détectés depuis le cache complet). Sélectionner un indexer re-fetche Prowlarr avec `?indexer=X` pour retourner les 100 meilleures releases de cet indexer spécifiquement.

## Releases

- **Indicateur d'échec** : badge sur les releases qui ont eu des erreurs de téléchargement précédentes
- **Blacklist** : masquez définitivement les releases indésirables (filtrées lors de la prochaine détection)
- Panel de blacklist avec possibilité de réafficher
- **Date de détection** (`detected_at`) : chaque release conserve la date de sa première découverte, même après un refresh

## Page Téléchargements

Tri disponible :

| Tri | Description |
|-----|-------------|
| **Récent** (défaut) | Par `detected_at` le plus récent parmi les releases de la série |
| Seeders | Par nombre de seeders du meilleur release |
| Manquants | Par nombre de volumes manquants |
| Nom | Par nom de série |

La date relative de détection est affichée sur chaque série (ex : "il y a 2h"). Ce tri est basé sur `detected_at` — la date de première découverte de la release — et non sur la date de mise à jour.
