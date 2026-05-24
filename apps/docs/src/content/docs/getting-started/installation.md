---
title: Installation
description: Comment installer Stripstream Librarian
---

## Prérequis système

4 outils sont requis pour le traitement des archives. Ils doivent être présents **dans le conteneur Docker** (ou sur la machine si vous installez manuellement) :

| Outil | Usage | Paquet |
|-------|-------|--------|
| `unrar` | Listing des CBR | `unrar` |
| `unar` | Extraction des CBR | `unar` / `theunarchiver` |
| `pdfinfo` | Comptage de pages PDF | `poppler-utils` |
| `pdftoppm` | Rendu de pages PDF | `poppler-utils` |

:::caution
`unrar` et `unar` sont deux outils différents issus de paquets distincts. Les deux sont nécessaires.
:::

Les images Docker officielles embarquent déjà ces dépendances — vous n'avez rien à installer séparément si vous utilisez Docker.

---

## Installation Docker (recommandé)

### 1 — Configurer l'environnement

```bash
cp .env.example .env
```

Éditez `.env` et changez au minimum :

| Variable | Valeur à définir |
|----------|-----------------|
| `API_BOOTSTRAP_TOKEN` | Un token secret long (ex. UUID) |
| `ADMIN_USERNAME` | Votre nom d'utilisateur admin |
| `ADMIN_PASSWORD` | Votre mot de passe admin |
| `SESSION_SECRET` | Chaîne aléatoire de 32+ caractères |

Les autres variables ont des valeurs par défaut fonctionnelles pour Docker.

### 2 — Configurer les dossiers

Par défaut, Docker crée ces dossiers à côté du `docker-compose.yml` :

| Variable `.env` | Dossier hôte par défaut | Monté dans le conteneur | Usage |
|----------------|------------------------|------------------------|-------|
| `LIBRARIES_HOST_PATH` | `./libraries` | `/libraries` | Vos fichiers CBZ/CBR/PDF/EPUB |
| `THUMBNAILS_HOST_PATH` | `./data/thumbnails` | `/data/thumbnails` | Miniatures générées |
| `DOWNLOADS_HOST_PATH` | `./data/downloads` | `/downloads` | Téléchargements qBittorrent |

Pour pointer vers une collection existante ailleurs sur le disque :

```bash
# Dans .env
LIBRARIES_HOST_PATH=/chemin/absolu/vers/ma/collection
```

:::tip
`LIBRARIES_ROOT_PATH` (valeur par défaut `/libraries`) est le chemin **à l'intérieur du conteneur**. Ne le changez pas si vous utilisez Docker. En développement local sans Docker, pointez-le vers votre dossier réel.
:::

### 3 — Lancer les services

```bash
docker compose up -d
```

Les services démarrent dans l'ordre correct (postgres → api → indexer + backoffice) grâce aux dépendances `healthcheck`. Le premier démarrage compile les images — patientez quelques minutes.

### Ports exposés

| Service | Port hôte | URL locale |
|---------|----------|-----------|
| Backoffice | `7082` | http://localhost:7082 |
| API | `7080` | http://localhost:7080 |
| Indexer | `7081` | http://localhost:7081 |
| Documentation | `7084` | http://localhost:7084 |
| PostgreSQL | `6432` | — |

Pour changer un port, modifiez directement `docker-compose.yml` (ex. `"8080:7080"` pour exposer l'API sur 8080).

---

## Installation manuelle

```bash
# Base de données PostgreSQL
docker compose up -d postgres

# Migrations
DATABASE_URL="postgres://stripstream:stripstream@localhost:6432/stripstream" \
  sqlx migrate run

# Backend (Rust)
cargo build --release

# Backoffice (Next.js)
cd apps/backoffice && npm install && npm run build
```

Démarrez ensuite les 3 services dans des terminaux séparés :
```bash
cargo run -p api
cargo run -p indexer
cd apps/backoffice && npm run start   # ou npm run dev en développement
```

---

## Premier lancement

Ouvrez le backoffice sur **http://localhost:7082** et connectez-vous avec les identifiants définis dans `.env` (`ADMIN_USERNAME` / `ADMIN_PASSWORD`).

### Démarrage rapide

1. **Créer une bibliothèque** — allez dans *Libraries*, donnez un nom et choisissez le dossier racine de vos fichiers (doit être dans le volume monté)
2. **Premier scan** — le scan démarre automatiquement. Les livres apparaissent en quelques secondes (phase discovery), les miniatures se génèrent ensuite en arrière-plan (phase analysis)
3. **Approuver des métadonnées** — allez dans *Series*, cliquez sur une série, cliquez sur **Rechercher des métadonnées**, sélectionnez un résultat et approuvez-le
4. C'est prêt — la série affiche sa couverture, sa description et les volumes manquants

Voir [Scan & Indexation](/libraries/scanning/) pour comprendre le pipeline en deux phases, et [Synchronisation des métadonnées](/metadata/sync/) pour le workflow de matching.
