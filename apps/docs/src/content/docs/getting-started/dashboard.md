---
title: Dashboard
description: Présentation du tableau de bord
---

Le dashboard est la page d'accueil du backoffice. Il donne une vue d'ensemble de votre bibliothèque en temps réel.

## Compteurs globaux

Six indicateurs en haut de page :

| Indicateur | Description |
|-----------|-------------|
| **Livres** | Nombre total de fichiers indexés |
| **Séries** | Nombre de séries distinctes |
| **Bibliothèques** | Nombre de bibliothèques créées |
| **Pages** | Cumul de toutes les pages de tous les livres |
| **Auteurs** | Nombre d'auteurs distincts |
| **Taille totale** | Volume occupé sur le disque |

## Lectures en cours et récentes

Si des livres ont un statut de lecture, deux listes s'affichent :

- **En cours de lecture** — livres avec statut `reading`, avec la page courante
- **Lus récemment** — derniers livres passés au statut `read`

Avec plusieurs utilisateurs, un sélecteur permet de filtrer par lecteur ou de voir tout le monde.

## Graphiques temporels

Trois graphiques avec sélecteur de période (jour / semaine / mois) :

| Graphique | Description |
|-----------|-------------|
| **Activité de lecture** | Livres ou pages lus dans le temps, par utilisateur |
| **Livres ajoutés** | Nouvelles entrées dans la bibliothèque au fil du temps |
| **Jobs** | Activité des jobs d'indexation par type |

## Répartition de la collection

| Graphique | Description |
|-----------|-------------|
| **Statut de lecture** | Répartition lu / en cours / non lu (par utilisateur si multi-users) |
| **Par format** | Proportion CBZ / CBR / PDF / EPUB |
| **Par bibliothèque** | Répartition des livres entre bibliothèques |

## Qualité des métadonnées

| Graphique | Description |
|-----------|-------------|
| **Couverture metadata** | Séries avec lien metadata approuvé vs sans |
| **Par provider** | Répartition des séries selon le provider utilisé |
| **Métadonnées livres** | % de livres avec résumé, % avec ISBN |

## Bibliothèques — détail

Barre empilée par bibliothèque montrant la progression de lecture (lu / en cours / non lu) avec la taille sur disque de chacune.

## Séries populaires

Top 8 des séries par nombre de livres, avec compteur de livres lus.

## Section téléchargements

Visible uniquement si les téléchargements sont activés et qu'il y a de l'activité. Affiche :
- Téléchargements actifs / importés / en erreur
- Séries avec volumes disponibles, total volumes manquants
- Historique récent des téléchargements avec statuts

## Liens rapides

En bas de page, raccourcis vers Libraries, Books, Series et Jobs.
