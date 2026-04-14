---
title: Configuration
description: Variables d'environnement et configuration
---

## Variables d'environnement

### Requises

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | URL de connexion PostgreSQL |
| `API_BOOTSTRAP_TOKEN` | Token admin initial |
| `ADMIN_USERNAME` | Nom d'utilisateur administrateur |
| `ADMIN_PASSWORD` | Mot de passe administrateur |
| `SESSION_SECRET` | Clé de session (min. 32 caractères) |

### Optionnelles

| Variable | Description | Défaut |
|----------|-------------|--------|
| `LIBRARIES_ROOT_PATH` | Remap des chemins bibliothèque | `/libraries/` |
| `RUST_LOG` | Niveaux de log | `indexer=info,scan=info,...` |

## Logging

Les domaines de logging disponibles :

| Domaine | Description |
|---------|-------------|
| `indexer` | Service d'indexation |
| `scan` | Scan de fichiers |
| `extraction` | Extraction de pages |
| `thumbnail` | Génération de miniatures |
| `watcher` | Surveillance filesystem |

Niveaux : `error`, `warn`, `info`, `debug`, `trace`

Exemple :
```bash
RUST_LOG="indexer=info,scan=debug,thumbnail=warn"
```

## Chemins des bibliothèques

Les chemins en base de données commencent par `/libraries/`. En développement local, utilisez `LIBRARIES_ROOT_PATH` pour remapper vers votre dossier réel.

:::tip
Les fonctions `remap_libraries_path()` et `unmap_libraries_path()` gèrent automatiquement la conversion des chemins.
:::
