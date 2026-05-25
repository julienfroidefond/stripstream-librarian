---
title: Wishlist
description: Constituer une liste de séries à acquérir et les télécharger automatiquement
---

La wishlist est un moyen de référencer des séries que vous souhaitez acquérir, sans avoir encore de fichiers locaux. Une série ajoutée à la wishlist :

- existe dans votre bibliothèque avec ses métadonnées complètes (description, couverture, auteurs, tomes)
- affiche **tous ses volumes comme manquants**
- est **éligible à la détection automatique de téléchargements** dès qu'elle est configurée

:::tip
La wishlist n'est pas une fonctionnalité séparée — c'est simplement ce que Stripstream appelle une **série sans livres locaux**. Toute série créée sans fichiers sur le disque se comporte comme une entrée de wishlist.
:::

---

## Ajouter une série à la wishlist

### Depuis la découverte (recommandé)

1. Ouvrez la page **Découverte**
2. Parcourez les onglets (Nouveautés, Meilleures, AniList, Prowlarr…)
3. Sur la carte d'une série qui vous intéresse, cliquez sur **Ajouter à la bibliothèque**
4. Sélectionnez la bibliothèque cible

La série est créée avec ses métadonnées synchronisées (description, couverture, auteurs, statut, liste des tomes) et un lien metadata approuvé — prête pour la détection.

### Manuellement

Sur la page **Séries**, le bouton **Nouvelle série** permet de créer une série en renseignant un nom et une bibliothèque, puis de lancer une recherche de métadonnées pour la lier à un provider.

---

## Consulter sa wishlist

Sur la page **Séries**, utilisez le filtre **Wishlist** (sélecteur "Livres") pour n'afficher que les séries sans livres locaux. Vous pouvez cumuler ce filtre avec un filtre de bibliothèque ou de recherche par nom.

---

## Workflow complet : de la découverte au téléchargement

```
1. Découverte
   Parcourez les listes → "Ajouter à la bibliothèque"
        │
        ▼
2. Série créée dans la wishlist
   0 livre local · metadata approuvé · tous les tomes = manquants
        │
        ▼
3. Détection automatique
   Job "download_detection" interroge Prowlarr
   pour chaque tome manquant de la série
        │
        ▼
4. Volumes disponibles
   Les releases trouvées apparaissent sur la page Téléchargements
   et sur la page de la série
        │
        ▼
5. Téléchargement & import
   Envoi à qBittorrent → import automatique → scan
   La série passe de wishlist à bibliothèque réelle
```

:::caution[Prérequis pour la détection]
Pour qu'une série wishlist soit cherchée dans Prowlarr, il faut :
1. Un **lien metadata approuvé** (automatique si ajouté depuis la découverte)
2. Des **volumes manquants** connus du provider (c'est le cas dès que `total_volumes > 0`)

Et dans les paramètres de la bibliothèque, la **détection automatique** doit être activée (ou lancer le job manuellement depuis la page Tâches).
:::

---

## Supprimer une entrée de la wishlist

Si vous ne souhaitez finalement pas acquérir une série, supprimez-la depuis sa page détail (menu **Plus → Supprimer la série**). Cela retire la série et ses métadonnées — aucun fichier n'est supprimé puisqu'il n'y en a pas.
