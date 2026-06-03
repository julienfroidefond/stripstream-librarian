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
| `RUST_LOG` | Niveau de verbosité des logs | `indexer=info,scan=info,...` |

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
