---
title: Ajouter à la bibliothèque
description: Ajouter des séries découvertes à votre bibliothèque
---

## Depuis la découverte

Quand vous ajoutez une série depuis la découverte :

1. Création de la série avec métadonnées (description, auteurs, genres, statut, cover)
2. Création d'un metadata link pour `bedetheque` et `senscritique`
3. Les providers `sc_*` (sc_trending_bd, sc_best_manga, etc.) sont normalisés vers `senscritique`
4. Pas de metadata link pour `anilist` et `prowlarr`
5. Revalidation des pages `/series` et `/libraries` après ajout

## Depuis l'interface série

`POST /series/create` — créer une série avec linking metadata optionnel en une seule requête :

- Sélecteur de bibliothèque + nom
- Recherche automatique de métadonnées
- Crée le répertoire physique sur disque
- Synchronise les métadonnées série + livres si un match provider est sélectionné
