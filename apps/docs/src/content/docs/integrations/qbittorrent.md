---
title: qBittorrent
description: Téléchargement et import automatique via qBittorrent
---

qBittorrent est le client de téléchargement **par défaut** utilisé par Stripstream pour récupérer les releases trouvées par Prowlarr. Les mêmes snatchs peuvent aussi être envoyés à [AllDebrid](/integrations/alldebrid), un débrideur qui télécharge le torrent sur ses serveurs — pratique sans client BitTorrent local.

## Configuration

Dans **Paramètres → onglet Outils de téléchargement → carte qBittorrent** :
- **URL** : adresse de votre instance qBittorrent (ex. `http://qbittorrent:8080`)
- **Nom d'utilisateur** et **mot de passe** : identifiants de l'interface qBittorrent
- **Activer qBittorrent** : interrupteur en haut à droite de la carte (activé par défaut). Désactivé, les snatchs ne sont plus envoyés à qBittorrent et le client n'apparaît plus comme sélectionnable dans le sélecteur *Client de téléchargement*.
- Un bouton **Tester la connexion** permet de vérifier que tout fonctionne

---

## Envoyer un torrent à qBittorrent

Depuis la page d'une série ou la page Téléchargements, cliquez sur le bouton de téléchargement à côté d'une release disponible. Le torrent est envoyé au client choisi dans *Paramètres → Outils de téléchargement → Client de téléchargement* : **qBittorrent** par défaut, ou [AllDebrid](/integrations/alldebrid) si vous l'avez sélectionné. La page Téléchargements rappelle le client utilisé en lecture seule.

**Mode Replace** : par défaut, Stripstream n'importe que les volumes manquants d'un torrent. En activant l'option **Replace**, tous les fichiers du torrent sont importés — y compris ceux que vous possédez déjà. Utile pour remplacer des fichiers par une version de meilleure qualité.

---

## Suivi et import automatique

Le suivi et l'import des torrents gérés sont toujours actifs, même si l'ancien réglage d'import
automatique avait été désactivé.

Une fois le téléchargement terminé dans qBittorrent, Stripstream le détecte automatiquement et importe les fichiers dans votre bibliothèque :

1. Les fichiers sont copiés dans le dossier de la série
2. Un scan est lancé pour les intégrer à la bibliothèque
3. Les métadonnées sont rafraîchies si la série est déjà liée à un provider

Vous pouvez suivre la progression en temps réel sur la page Téléchargements.

---

## Gestion des téléchargements en erreur

Si un import échoue, vous pouvez le **relancer** depuis la page Téléchargements. Stripstream vérifie que les fichiers source existent toujours avant de retenter.

**Torrent bloqué ou en pause** : un torrent mis en pause dans qBittorrent n'est jamais considéré comme en échec. Un torrent bloqué faute de sources (`stalledDL`) n'est passé en erreur qu'après le délai de blocage configuré (réglage AllDebrid **Délai avant échec d'un magnet bloqué**, 30 min par défaut, qui s'applique aussi à qBittorrent) ; le délai repart de zéro dès que le téléchargement reprend. Un torrent supprimé à la main dans qBittorrent passe en erreur au lieu de rester « en cours ».

**Import partiel** : quand tous les volumes n'ont pas pu être importés (`partial`, `no_files_imported`), les fichiers source et le torrent sont conservés pour qu'un nouvel essai puisse compléter l'import. Ils sont nettoyés quand l'import est complet ou quand vous supprimez la ligne.

**Torrent déjà présent** : si le torrent existait déjà dans qBittorrent (ajouté par vous, hors de la catégorie Stripstream), Stripstream importe ses fichiers mais ne supprime jamais le torrent lui-même.

:::note[Détails techniques]
**Détection des doublons** : si un torrent est déjà présent dans qBittorrent (détection par hash magnet), Stripstream lit le `content_path` réel et lance directement l'import si le torrent est déjà complété.

**Pipeline d'import** : extraction des volumes depuis les noms de fichiers (supporte `Tome_01` avec underscore) ; matching de série via `norm_text()` ; nommage des fichiers depuis le livre de référence existant ; déduplication par format (cbz > cbr > pdf > epub) ; one-shots importés tels quels (sans numéro de volume).

**Statuts de téléchargement** : `importing`, `imported`, `partial`, `no_files_imported`, `failed`. Les états qBittorrent 5.x `stoppedUP` (terminé) et `stoppedDL` (en pause) sont reconnus. Un retry sur une ligne déjà en cours d'import est refusé (409).

**Cleanup** : suppression du torrent et de son répertoire de téléchargement après import réussi. Les anciens répertoires `sl-*` temporaires sont nettoyés.

**Retry** : vérifie que les fichiers source existent avant de relancer (évite les imports bloqués en boucle).

**Webhook de fin de téléchargement** : l'appel `POST /torrent-downloads/notify` envoyé par qBittorrent n'accepte que les chemins situés dans le dossier de téléchargements (`/downloads`). Tout chemin hors de ce dossier (ou contenant une traversée `..`) est refusé, afin qu'un appel non authentifié ne puisse pas diriger l'import vers un autre emplacement du système de fichiers.
:::
