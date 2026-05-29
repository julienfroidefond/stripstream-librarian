---
title: Ajouter à la bibliothèque
description: Ajouter des séries découvertes à votre bibliothèque
---

## Depuis les tendances

Quand vous cliquez sur "+" sur une carte de découverte (tendances, AniList, Bédéthèque…) :

1. Création de la série avec métadonnées (description, auteurs, genres, statut, cover)
2. Création d'un metadata link pour `bedetheque` et `senscritique`
3. Les providers `sc_*` (sc_trending_bd, sc_best_manga, etc.) sont normalisés vers `senscritique`
4. Pas de metadata link pour `anilist`
5. Revalidation des pages `/series` et `/libraries` après ajout

## Depuis Prowlarr

L'ajout depuis l'onglet Prowlarr passe par un modal dédié en plusieurs étapes :

1. **Bibliothèque** — sélection de la bibliothèque cible (ignorée s'il n'y en a qu'une)
2. **Recherche** — deux recherches parallèles sur la série pré-remplie :
   - Séries déjà présentes dans la bibliothèque (correspondance par nom)
   - Métadonnées externes (tous les providers)
3. **Action** selon la sélection :
   - *Série existante sélectionnée* → "Télécharger → [nom]" : lance le téléchargement vers la série existante, sans créer de doublon
   - *Candidat metadata sélectionné* → "Ajouter seulement" ou "Ajouter et télécharger" : crée la série avec le metadata link du provider choisi
   - *Aucune sélection* → "Ajouter seulement" ou "Ajouter et télécharger" : crée la série sans metadata link (provider `prowlarr`)

Le téléchargement est envoyé à qBittorrent avec `series_name` et `expected_volumes` issus de la release Prowlarr.

![Modal d'ajout Prowlarr — recherche de métadonnées](/screenshots/prowlarr-add-modal.png)

![Modal d'ajout Prowlarr — série existante sélectionnée](/screenshots/prowlarr-add-modal-existing.png)

## Depuis l'interface série

`POST /series/create` — créer une série avec linking metadata optionnel en une seule requête :

- Sélecteur de bibliothèque + nom
- Recherche automatique de métadonnées
- Crée le répertoire physique sur disque
- Synchronise les métadonnées série + livres si un match provider est sélectionné
