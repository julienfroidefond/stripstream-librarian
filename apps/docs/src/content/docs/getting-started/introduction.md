---
title: Introduction
description: Présentation de Stripstream Librarian
---

**Stripstream Librarian** est un gestionnaire de bibliothèque de bandes dessinées et ebooks auto-hébergé. Il vous permet de gérer, organiser et enrichir automatiquement vos collections de BD, mangas et ebooks.

## Fonctionnalités principales

- **Multi-bibliothèques** : créez et gérez plusieurs bibliothèques indépendantes
- **Multi-formats** : CBZ, CBR, PDF, EPUB supportés nativement
- **Indexation automatique** : scan en deux phases (découverte rapide + analyse approfondie)
- **Métadonnées externes** : 6 providers pour enrichir automatiquement vos séries
- **Découverte** : parcourez les tendances et tops depuis SensCritique, AniList, Bedetheque
- **Intégrations** : Prowlarr, qBittorrent, AniList, Komga, Telegram
- **Interface web** : backoffice complet avec dashboard, recherche, gestion des séries

## Architecture

| Service | Description | Port |
|---------|-------------|------|
| **API** | API REST (axum) | 7080 |
| **Indexer** | Service d'indexation en arrière-plan | 7081 |
| **Backoffice** | Interface web (Next.js) | 7082 |
| **PostgreSQL** | Base de données | 6432 |

## Stack technique

- **Backend** : Rust (axum, sqlx, tokio)
- **Frontend** : Next.js 16 / React 19 / Tailwind CSS
- **Base de données** : PostgreSQL avec `pg_trgm` pour la recherche
- **Formats** : parsers dédiés pour CBZ, CBR, PDF, EPUB
