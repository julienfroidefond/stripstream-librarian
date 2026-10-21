---
title: Configuration
description: Variables d'environnement et configuration
---

## Configuration initiale

La configuration de Stripstream se fait principalement via le fichier `.env` avant le premier démarrage. Copiez `.env.example` en `.env` et renseignez les valeurs requises.

### Variables requises

Ces variables doivent obligatoirement être définies — Stripstream ne démarrera pas sans elles :

| Variable | Ce qu'elle représente |
|----------|----------------------|
| `DATABASE_URL` | Adresse de connexion à la base de données |
| `API_BOOTSTRAP_TOKEN` | Token secret pour le premier accès admin |
| `ADMIN_USERNAME` | Nom d'utilisateur du compte administrateur |
| `ADMIN_PASSWORD` | Mot de passe du compte administrateur |
| `SESSION_SECRET` | Clé secrète pour les sessions (min. 32 caractères) |

### Variables optionnelles

| Variable | Description | Valeur par défaut |
|----------|-------------|-------------------|
| `LIBRARIES_ROOT_PATH` | Chemin de montage des bibliothèques dans le conteneur | `/libraries/` |
| `DOWNLOADS_PATH` | Chemin local correspondant à `/downloads` (développement hors Docker) | non remappé |
| `IMAGE_CACHE_DIR` | Répertoire de repli du cache disque des pages | `/tmp/stripstream-image-cache` |
| `API_LISTEN_ADDR` | Adresse d'écoute de l'API | `0.0.0.0:7080` |
| `API_BASE_URL` | URL de l'API vue par le backoffice | `http://api:7080` |
| `INDEXER_LISTEN_ADDR` | Adresse d'écoute de l'indexer | `0.0.0.0:7081` |
| `INDEXER_SCAN_INTERVAL_SECONDS` | Intervalle de vérification du watcher | `5` |
| `RUST_LOG` | Niveau de verbosité des logs | `indexer=info,scan=info,...` |

La liste exhaustive (volumes Docker, pool PostgreSQL, logs détaillés) est dans
[Variables d'environnement](/reference/environment/).

:::tip
En production Docker, ne modifiez pas `LIBRARIES_ROOT_PATH`. Cette variable n'est utile qu'en développement local sans Docker, pour pointer vers le dossier réel de vos fichiers.
:::

---

## Niveaux de logs

Si vous souhaitez obtenir plus de détails dans les logs (par exemple pour diagnostiquer un problème de scan), ajustez `RUST_LOG` dans votre `.env` :

```bash
# Plus de détails sur le scan
RUST_LOG="indexer=info,scan=debug,thumbnail=warn"

# Tout en debug (très verbeux)
RUST_LOG="debug"
```

Les niveaux disponibles, du plus silencieux au plus verbeux : `error`, `warn`, `info`, `debug`, `trace`.

:::note[Détails techniques — domaines de log]
| Domaine | Description |
|---------|-------------|
| `indexer` | Service d'indexation |
| `scan` | Scan de fichiers |
| `extraction` | Extraction de pages |
| `thumbnail` | Génération de miniatures |
| `watcher` | Surveillance filesystem |
:::
