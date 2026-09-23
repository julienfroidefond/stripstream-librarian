---
title: Lacunes de métadonnées
description: Repérer et corriger les métadonnées manquantes sur vos séries et vos livres
---

La page **Données → Métadonnées** liste tout ce qui manque dans votre bibliothèque : séries sans description, livres sans résumé, tomes sans ISBN, etc. Elle sert de tableau de bord pour savoir *où* concentrer vos efforts d'enrichissement, plutôt que de parcourir les séries une par une.

## Deux onglets

La page est divisée en deux onglets :

| Onglet | Portée | Champs surveillés |
|--------|--------|-------------------|
| **Séries** | Une ligne par série | Description, genres, auteurs, éditeurs, année de début, couverture |
| **Livres** | Une ligne par livre | Résumé, ISBN, couverture, auteur, date de publication, langue, numéro de volume |

Chaque onglet affiche un tableau avec les colonnes pertinentes (nom, fournisseur de métadonnées, statut, année, genres, tomes, manquants, note pour les séries ; titre, série, tome, auteurs, langue, pages, format, résumé, ISBN pour les livres).

## Filtrer par lacune

Au-dessus du tableau, une rangée de **puces** permet de filtrer sur une lacune précise. Chaque puce affiche le nombre d'éléments concernés :

- **Toutes** — aucun filtre, la liste complète
- **Sans description**, **Sans genre**, **Sans auteur**, **Sans éditeur**, **Sans année**, **Sans couverture** (onglet Séries)
- **Sans résumé**, **Sans ISBN**, **Sans couverture**, **Sans auteur**, **Sans date de publication**, **Sans langue**, **Sans numéro de volume** (onglet Livres)

Cliquer sur une puce filtre le tableau ; recliquer sur **Toutes** retire le filtre.

## Recherche et bibliothèque

Le formulaire de recherche permet de :

- **Rechercher** un titre, une série ou un auteur (recherche plein texte)
- **Restreindre à une bibliothèque** via le sélecteur

Les deux filtres se combinent avec la puce de lacune sélectionnée.

## Corriger une lacune

Chaque ligne du tableau renvoie vers la série ou le livre concerné. Depuis la page de détail, vous pouvez :

- **Lancer une recherche de métadonnées** et approuver un match (voir [Providers de métadonnées](/metadata/providers))
- **Éditer manuellement** les champs et les verrouiller (voir [Métadonnées des livres](/books/metadata))
- **Lancer un batch** pour lier automatiquement les séries non liées (voir [Batch & Refresh](/metadata/batch-refresh))

:::note[Détails techniques]
**Endpoint** : `GET /metadata/gaps/summary` (scope `read`), avec un paramètre optionnel `library_id`. Il renvoie 15 compteurs agrégés (7 pour les séries, 8 pour les livres).

**Filtres de liste** : les endpoints `GET /series` et `GET /books` acceptent un paramètre `gap=` :

| Endpoint | Valeurs de `gap` |
|----------|------------------|
| `GET /series` | `no_description`, `no_genre`, `no_authors`, `no_publishers`, `no_year`, `no_cover` |
| `GET /books` | `no_summary`, `no_isbn`, `no_cover`, `no_author`, `no_publish_date`, `no_language`, `no_volume` |

Une valeur inconnue est ignorée (aucun filtre appliqué). Les compteurs du résumé utilisent **exactement les mêmes prédicats SQL** que les filtres de liste, donc les puces et les tableaux filtrés ne peuvent jamais diverger.

**Définition des lacunes** :

- *Sans couverture* (série) : `cover_url` nul ou vide ; (livre) : `thumbnail_path` nul
- *Sans auteur* (livre) : ni `authors` (tableau non vide) ni `author` (chaîne non vide)
- *Sans numéro de volume* : `volume` nul
:::
