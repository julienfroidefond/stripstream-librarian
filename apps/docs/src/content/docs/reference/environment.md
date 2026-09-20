---
title: Variables d'environnement
description: Référence complète des variables d'environnement
---

## Variables requises

Ces variables doivent être définies avant le premier démarrage. Stripstream refuse de démarrer si elles sont absentes.

| Variable | Ce qu'elle représente |
|----------|----------------------|
| `DATABASE_URL` | Adresse de connexion à la base de données |
| `API_BOOTSTRAP_TOKEN` | Secret utilisé pour créer le premier accès admin |
| `ADMIN_USERNAME` | Nom d'utilisateur du compte administrateur |
| `ADMIN_PASSWORD` | Mot de passe du compte administrateur |
| `SESSION_SECRET` | Clé secrète pour les sessions (minimum 32 caractères) |

:::note[Détails techniques]
Exemples :
- `DATABASE_URL=postgres://user:pass@localhost:6432/stripstream`
- `API_BOOTSTRAP_TOKEN` peut être un UUID ou toute chaîne longue et aléatoire.
:::

## Chemins

| Variable | Description | Valeur par défaut |
|----------|-------------|------------------|
| `LIBRARIES_ROOT_PATH` | Chemin de montage des bibliothèques dans le conteneur | `/libraries/` |

:::note[Détails techniques]
Les chemins de fichiers stockés en base de données commencent par `/libraries/`. En développement local sans Docker, définissez `LIBRARIES_ROOT_PATH` pour pointer vers le dossier réel de vos fichiers.
:::

## API

| Variable | Description | Valeur par défaut |
|----------|-------------|------------------|
| `API_LISTEN_ADDR` | Adresse d'écoute du service API | `0.0.0.0:7080` |
| `API_DB_MAX_CONNECTIONS` | Taille maximale du pool de connexions PostgreSQL | `10` |

## Niveaux de log

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Niveau de détail des logs |

Augmentez ce niveau uniquement pour diagnostiquer un problème précis : plus les logs sont détaillés, plus ils peuvent être volumineux.

:::note[Détails techniques]
Niveaux disponibles (du moins au plus verbeux) : `error` < `warn` < `info` < `debug` < `trace`

Valeur par défaut : `indexer=info,scan=info,extraction=info,thumbnail=warn,watcher=info`

Domaines disponibles : `indexer` (service principal), `scan` (scan de fichiers), `extraction` (extraction de pages), `thumbnail` (génération de miniatures), `watcher` (surveillance filesystem).

Exemple : `RUST_LOG=indexer=info,scan=debug`
:::
