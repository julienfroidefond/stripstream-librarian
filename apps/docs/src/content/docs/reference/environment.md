---
title: Variables d'environnement
description: Référence complète des variables d'environnement
---

## Variables requises

Ces variables doivent être définies avant le premier démarrage. Stripstream refuse de démarrer si elles sont absentes.

| Variable | Ce qu'elle représente |
|----------|----------------------|
| `DATABASE_URL` | Adresse de connexion PostgreSQL (ex: `postgres://user:pass@localhost:6432/stripstream`) |
| `API_BOOTSTRAP_TOKEN` | Token secret pour le premier accès admin (ex: un UUID) |
| `ADMIN_USERNAME` | Nom d'utilisateur du compte administrateur |
| `ADMIN_PASSWORD` | Mot de passe du compte administrateur |
| `SESSION_SECRET` | Clé secrète pour les sessions (minimum 32 caractères) |

## Chemins

| Variable | Description | Valeur par défaut |
|----------|-------------|------------------|
| `LIBRARIES_ROOT_PATH` | Chemin de montage des bibliothèques dans le conteneur | `/libraries/` |

:::note
Les chemins de fichiers stockés en base de données commencent par `/libraries/`. En développement local sans Docker, définissez `LIBRARIES_ROOT_PATH` pour pointer vers le dossier réel de vos fichiers.
:::

## Niveaux de log

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Niveaux de verbosité par domaine (ex: `indexer=info,scan=debug`) |

Niveaux disponibles (du moins au plus verbeux) : `error` < `warn` < `info` < `debug` < `trace`

Valeur par défaut : `indexer=info,scan=info,extraction=info,thumbnail=warn,watcher=info`

Domaines disponibles : `indexer` (service principal), `scan` (scan de fichiers), `extraction` (extraction de pages), `thumbnail` (génération de miniatures), `watcher` (surveillance filesystem).
