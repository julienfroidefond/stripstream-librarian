---
title: Introduction
description: Présentation de Stripstream Librarian
---

**Stripstream Librarian** est un gestionnaire de bibliothèque de bandes dessinées et ebooks auto-hébergé. Il vous permet de gérer, organiser et enrichir automatiquement vos collections de BD, mangas et ebooks.

## Ce que vous pouvez faire avec Stripstream

- **Gérer plusieurs bibliothèques** : organisez vos collections en bibliothèques indépendantes (mangas, BD franco-belge, comics…)
- **Lire tous les formats courants** : CBZ, CBR, PDF et EPUB sont supportés nativement
- **Indexer automatiquement** : ajoutez des fichiers dans un dossier, ils apparaissent dans la bibliothèque en quelques secondes
- **Enrichir avec des métadonnées** : récupérez automatiquement descriptions, couvertures, auteurs et liste des tomes depuis 6 sources externes (AniList, SensCritique, Bédéthèque, ComicVine, Google Books, Open Library)
- **Découvrir de nouvelles séries** : parcourez les tendances et tops depuis SensCritique, AniList et Bédéthèque
- **Télécharger automatiquement** : détectez les volumes manquants, téléchargez-les via qBittorrent, et importez-les automatiquement dans la bibliothèque
- **Suivre votre progression de lecture** : marquez vos livres tome par tome, synchronisez avec AniList
- **Interface web complète** : dashboard, recherche, gestion des séries, des genres, des listes de lecture

## Premiers pas

1. [Installez Stripstream](/getting-started/installation/) avec Docker en quelques minutes
2. [Créez votre première bibliothèque](/libraries/management/) et lancez un scan
3. [Enrichissez vos séries](/metadata/sync/) avec des métadonnées externes

:::note[Détails techniques]
Stripstream est composé de plusieurs services :

| Service | Description | Port |
|---------|-------------|------|
| **API** | API REST (Rust/axum) | 7080 |
| **Indexer** | Service d'indexation en arrière-plan | 7081 |
| **Backoffice** | Interface web (Next.js) | 7082 |
| **PostgreSQL** | Base de données | 6432 |

Stack : backend Rust (axum, sqlx, tokio), frontend Next.js 16 / React 19 / Tailwind CSS, base de données PostgreSQL avec `pg_trgm` pour la recherche full-text.
:::
