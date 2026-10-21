---
title: Variables d'environnement
description: Référence complète des variables d'environnement
---

## Variables requises

Ces variables doivent être définies avant le premier démarrage. Stripstream refuse de démarrer si elles sont absentes.

| Variable | Ce qu'elle représente |
|----------|----------------------|
| `DATABASE_URL` | Adresse de connexion à la base de données |
| `API_BOOTSTRAP_TOKEN` | Secret utilisé pour créer le premier accès admin (et par le backoffice pour appeler l'API) |
| `ADMIN_USERNAME` | Nom d'utilisateur du compte administrateur (backoffice) |
| `ADMIN_PASSWORD` | Mot de passe du compte administrateur (backoffice) |
| `SESSION_SECRET` | Clé secrète pour les sessions du backoffice (minimum 32 caractères) |

:::note[Détails techniques]
Exemples :
- `DATABASE_URL=postgres://user:pass@localhost:6432/stripstream`
- `API_BOOTSTRAP_TOKEN` peut être un UUID ou toute chaîne longue et aléatoire.
:::

## Services

| Variable | Description | Valeur par défaut |
|----------|-------------|------------------|
| `API_LISTEN_ADDR` | Adresse d'écoute du service API | `0.0.0.0:7080` |
| `API_DB_MAX_CONNECTIONS` | Taille maximale du pool de connexions PostgreSQL | `10` |
| `API_BASE_URL` | URL de l'API vue par le backoffice | `http://api:7080` |
| `INDEXER_LISTEN_ADDR` | Adresse d'écoute du service Indexer | `0.0.0.0:7081` |
| `INDEXER_SCAN_INTERVAL_SECONDS` | Intervalle de vérification du watcher filesystem | `5` |

## Chemins

| Variable | Description | Valeur par défaut |
|----------|-------------|------------------|
| `LIBRARIES_ROOT_PATH` | Chemin de montage des bibliothèques dans le conteneur | `/libraries/` |
| `DOWNLOADS_PATH` | Chemin local correspondant à `/downloads` (développement hors Docker) | non remappé |
| `IMAGE_CACHE_DIR` | Répertoire de départ du cache disque des pages, avant tout réglage en base | `/tmp/stripstream-image-cache` |

:::note[Détails techniques]
Les chemins de fichiers stockés en base de données commencent par `/libraries/` (et `/downloads` pour les téléchargements). En développement local sans Docker, définissez `LIBRARIES_ROOT_PATH` et `DOWNLOADS_PATH` pour pointer vers les dossiers réels.

Le répertoire du cache de pages et les réglages des miniatures (format, dimensions, qualité, répertoire) se modifient **à chaud** dans **Paramètres → Images & cache** : ces valeurs en base prennent le dessus sur `IMAGE_CACHE_DIR`, qui ne sert que de repli au premier démarrage. Les variables `THUMBNAIL_*` ne pilotent pas la génération des miniatures — configurez-les dans l'interface.
:::

## Volumes Docker

Ces variables ne sont pas lues par les services : elles sont interpolées par `docker-compose.yml` pour définir les dossiers de l'hôte montés dans les conteneurs.

| Variable | Description | Valeur par défaut |
|----------|-------------|------------------|
| `LIBRARIES_HOST_PATH` | Dossier hôte des bibliothèques | `./libraries` |
| `THUMBNAILS_HOST_PATH` | Dossier hôte des miniatures | `./data/thumbnails` |
| `DOWNLOADS_HOST_PATH` | Dossier hôte des téléchargements | `./data/downloads` |

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
