---
title: qBittorrent
description: Téléchargement et import automatique via qBittorrent
---

qBittorrent est le client de téléchargement utilisé par Stripstream pour récupérer les releases trouvées par Prowlarr.

## Configuration

Dans **Settings → Download tools → qBittorrent** :
- **URL** : adresse de votre instance qBittorrent (ex. `http://qbittorrent:8080`)
- **Nom d'utilisateur** et **mot de passe** : identifiants de l'interface qBittorrent
- Un bouton **Tester la connexion** permet de vérifier que tout fonctionne

---

## Envoyer un torrent à qBittorrent

Depuis la page d'une série ou la page Téléchargements, cliquez sur le bouton de téléchargement à côté d'une release disponible. Le torrent est envoyé automatiquement à qBittorrent.

**Mode Replace** : par défaut, Stripstream n'importe que les volumes manquants d'un torrent. En activant l'option **Replace**, tous les fichiers du torrent sont importés — y compris ceux que vous possédez déjà. Utile pour remplacer des fichiers par une version de meilleure qualité.

---

## Suivi et import automatique

Une fois le téléchargement terminé dans qBittorrent, Stripstream le détecte automatiquement et importe les fichiers dans votre bibliothèque :

1. Les fichiers sont copiés dans le dossier de la série
2. Un scan est lancé pour les intégrer à la bibliothèque
3. Les métadonnées sont rafraîchies si la série est déjà liée à un provider

Vous pouvez suivre la progression en temps réel sur la page Téléchargements.

---

## Gestion des téléchargements en erreur

Si un import échoue, vous pouvez le **relancer** depuis la page Téléchargements. Stripstream vérifie que les fichiers source existent toujours avant de retenter.

:::note[Détails techniques]
**Détection des doublons** : si un torrent est déjà présent dans qBittorrent (détection par hash magnet), Stripstream lit le `content_path` réel et lance directement l'import si le torrent est déjà complété.

**Pipeline d'import** : extraction des volumes depuis les noms de fichiers (supporte `Tome_01` avec underscore) ; matching de série via `norm_text()` ; nommage des fichiers depuis le livre de référence existant ; déduplication par format (cbz > cbr > pdf > epub) ; one-shots importés tels quels (sans numéro de volume).

**Statuts de téléchargement** : `importing`, `imported`, `partial`, `no_files_imported`, `failed`.

**Cleanup** : suppression du torrent et de son répertoire de téléchargement après import réussi. Les anciens répertoires `sl-*` temporaires sont nettoyés.

**Retry** : vérifie que les fichiers source existent avant de relancer (évite les imports bloqués en boucle).

**Webhook de fin de téléchargement** : l'appel `POST /torrent-downloads/notify` envoyé par qBittorrent n'accepte que les chemins situés dans le dossier de téléchargements (`/downloads`). Tout chemin hors de ce dossier (ou contenant une traversée `..`) est refusé, afin qu'un appel non authentifié ne puisse pas diriger l'import vers un autre emplacement du système de fichiers.
:::
