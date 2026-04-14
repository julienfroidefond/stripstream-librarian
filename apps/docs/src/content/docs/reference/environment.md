---
title: Variables d'environnement
description: Référence complète des variables d'environnement
---

## Variables requises

| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | URL PostgreSQL (ex: `postgres://user:pass@localhost:6432/stripstream`) |
| `API_BOOTSTRAP_TOKEN` | Token admin pour le premier accès |
| `ADMIN_USERNAME` | Nom d'utilisateur admin |
| `ADMIN_PASSWORD` | Mot de passe admin |
| `SESSION_SECRET` | Clé de session (minimum 32 caractères) |

## Chemins

| Variable | Description | Défaut |
|----------|-------------|--------|
| `LIBRARIES_ROOT_PATH` | Préfixe de remap des chemins bibliothèque | `/libraries/` |

:::note
Les chemins en DB commencent par `/libraries/`. En dev local, `LIBRARIES_ROOT_PATH` permet de remapper vers le dossier réel.
:::

## Logging

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Niveaux de log par domaine |

### Domaines

| Domaine | Description |
|---------|-------------|
| `indexer` | Service d'indexation principal |
| `scan` | Scan de fichiers |
| `extraction` | Extraction de pages |
| `thumbnail` | Génération de miniatures |
| `watcher` | Surveillance filesystem |

### Niveaux

`error` < `warn` < `info` < `debug` < `trace`

### Défaut

```
indexer=info,scan=info,extraction=info,thumbnail=warn,watcher=info
```
