---
title: Ajouter à la bibliothèque
description: Ajouter des séries découvertes à votre bibliothèque
---

## Depuis les tendances

Quand vous cliquez sur "+" sur une carte de découverte (tendances SensCritique, AniList…), Stripstream crée immédiatement la série avec ses métadonnées complètes : description, auteurs, genres, statut, couverture, et liste des tomes.

La série est directement prête pour la détection de volumes manquants.

---

## Depuis Prowlarr

L'ajout depuis l'onglet Prowlarr passe par un assistant dédié en plusieurs étapes :

1. **Bibliothèque** — sélection de la bibliothèque cible (ignorée s'il n'y en a qu'une)
2. **Recherche** — deux recherches parallèles sur le nom de la série pré-rempli :
   - Séries déjà présentes dans la bibliothèque
   - Métadonnées disponibles sur tous les providers
3. **Action** selon votre choix :
   - *Série existante sélectionnée* → lance le téléchargement vers la série existante, sans créer de doublon
   - *Résultat metadata sélectionné* → crée la série avec ce lien metadata, avec option "Ajouter et télécharger"
   - *Aucune sélection* → crée la série sans lien metadata, avec option "Ajouter et télécharger"

![Modal d'ajout Prowlarr — recherche de métadonnées](/screenshots/prowlarr-add-modal.png)

![Modal d'ajout Prowlarr — série existante sélectionnée](/screenshots/prowlarr-add-modal-existing.png)

---

## Créer une série manuellement

Sur la page **Séries**, le bouton **Nouvelle série** vous permet de :

1. Choisir une bibliothèque et saisir un nom
2. Lancer une recherche de métadonnées pour associer la série à un provider
3. Créer la série avec un répertoire physique sur le disque

:::note[Détails techniques]
**Depuis les tendances** : pour SensCritique, un metadata link est créé automatiquement. AniList ne crée pas de metadata link lors de l'ajout depuis la découverte. Les providers `sc_*` (sc_trending_bd, sc_best_manga, etc.) sont normalisés vers `senscritique`.

**Depuis Prowlarr** : le téléchargement est envoyé à qBittorrent avec `series_name` et `expected_volumes` issus de la release Prowlarr.

**API création** : `POST /series/create` — crée la série, crée le répertoire physique, synchronise les métadonnées si un match provider est sélectionné. Revalide les pages `/series` et `/libraries` après ajout.
:::
