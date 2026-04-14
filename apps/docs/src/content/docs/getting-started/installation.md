---
title: Installation
description: Comment installer Stripstream Librarian
---

## Prérequis

### Dépendances système
4 outils sont requis pour le traitement des archives :

| Outil | Usage | Paquet |
|-------|-------|--------|
| `unrar` | Listing des CBR | `unrar` |
| `unar` | Extraction des CBR | `unar` / `theunarchiver` |
| `pdfinfo` | Comptage de pages PDF | `poppler-utils` |
| `pdftoppm` | Rendu de pages PDF | `poppler-utils` |

:::caution
`unrar` et `unar` sont deux outils différents issus de paquets distincts. Les deux sont nécessaires.
:::

### Docker (recommandé)

```bash
docker compose up -d
```

Le `docker-compose.yml` à la racine du projet lance tous les services.

### Installation manuelle

```bash
# Base de données
docker compose up -d postgres

# Backend
cargo build --release

# Backoffice
cd apps/backoffice && npm install && npm run build
```

## Premier lancement

1. Copiez le fichier d'environnement :
   ```bash
   cp .env.example .env
   ```

2. Configurez les variables requises (voir [Configuration](/getting-started/configuration/))

3. Lancez les migrations :
   ```bash
   sqlx migrate run
   ```

4. Démarrez les services :
   ```bash
   # API
   cargo run -p api
   # Indexer
   cargo run -p indexer
   # Backoffice
   cd apps/backoffice && npm run dev
   ```
