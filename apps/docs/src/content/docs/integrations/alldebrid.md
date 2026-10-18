---
title: AllDebrid
description: Client de téléchargement alternatif à qBittorrent — débridage et import automatique
---

AllDebrid est un service de *débridage* : ses serveurs téléchargent le torrent à votre place et
exposent les fichiers en liens directs. Stripstream peut l'utiliser comme **client de
téléchargement alternatif à [qBittorrent](/integrations/qbittorrent)** pour les snatchs trouvés par
[Prowlarr](/integrations/prowlarr) — utile quand vous ne voulez (ou ne pouvez) pas faire tourner un
client BitTorrent local, semer, ou passer par un VPN.

Seul le téléchargement change : tout ce qui suit (matching des volumes, renommage, déduplication,
scan, rafraîchissement des métadonnées) est le pipeline d'import commun, identique à celui de
qBittorrent.

## Configuration

Dans **Settings → Download tools → AllDebrid** :
- **Clé API** : votre clé AllDebrid (AllDebrid → *Mon compte* → clé API). Un compte premium ou en
  essai est requis : un compte gratuit est refusé par le test de connexion.
- **Activer AllDebrid** : *Enabled*

Le bouton **Tester la connexion** vérifie la clé et affiche le compte connecté.

:::note
La clé API n'est **jamais réaffichée** après l'enregistrement : le champ reste vide, avec le libellé
« clé enregistrée ». Laissez-le vide pour conserver la clé existante, saisissez-en une nouvelle pour
la remplacer. La clé n'est renvoyée par aucun endpoint — `GET /settings/alldebrid` la masque et la
page [Diagnostic](/getting-started/diagnostics) ne l'expose pas non plus.
:::

## Envoyer un snatch à AllDebrid

Le parcours est inchangé : depuis une release disponible (page Téléchargements, recherche Prowlarr
d'une série, détection de téléchargements d'une bibliothèque), le bouton de téléchargement envoie la
release au **client sélectionné**.

qBittorrent et AllDebrid sont des alternatives, jamais cumulatives : le client qui reçoit les
snatchs se choisit **une seule fois**, dans *Paramètres → Outils de téléchargement → **Client de
téléchargement***. Aucune modale ne le demande. La page *Téléchargements* n'affiche plus qu'un
rappel en lecture seule (`Client : AllDebrid`). Chaque ligne de l'historique affiche un badge
indiquant le client qui l'a traitée, et les messages d'envoi nomment le client (« Envoyer à
AllDebrid »).

Le **mode Replace** se comporte comme avec qBittorrent : tous les fichiers du torrent sont importés,
pas seulement les volumes manquants.

## Déroulement

1. Stripstream envoie le magnet (ou le fichier `.torrent`) à AllDebrid.
2. AllDebrid télécharge sur ses serveurs ; le nom du torrent, la progression, la vitesse, l'ETA et le
   nombre de seeders remontent dans l'historique. Le nom s'affiche sous la ligne de série, comme pour
   les téléchargements Telegram et directs.
3. Une fois le magnet terminé, les fichiers de livre sont récupérés en streaming dans un répertoire
   temporaire (`/downloads/sl-<id>`) puis le magnet est supprimé d'AllDebrid.
4. L'import enchaîne le pipeline habituel : extraction des volumes, matching de série, nommage,
   déduplication par format, scan et refresh des métadonnées.

Les statuts de l'historique sont les mêmes que pour qBittorrent (En cours, Terminé, Import en cours,
Importé, Partiel, Aucun fichier importé, Erreur) — voir [Téléchargements](/downloads/overview).

## Magnet sans pair

Un torrent sans seeder ne démarre jamais : AllDebrid reste à 0 octet indéfiniment et la ligne
tournerait « En cours » sans fin. Pour éviter cela, Stripstream abandonne automatiquement un magnet qui
n'a téléchargé **aucun octet** après **30 minutes**, avec le message explicite
`No peer: 0 bytes downloaded in 30 min (0 seeders)`. Le compteur de seeders est affiché en orange
lorsqu'il est à zéro, ce qui permet de repérer le problème avant l'abandon.

Le magnet n'est pas supprimé d'AllDebrid : **Retry** réarme la fenêtre de surveillance, ce qui laisse
30 minutes supplémentaires au torrent pour trouver des pairs (AllDebrid refuse de « relancer » un
magnet qu'il considère déjà actif). Si le torrent reste sans pair, envisagez une autre release ou un
autre client (qBittorrent).

## Relancer un téléchargement en erreur

Le bouton **Retry (↺)** de l'historique s'adapte au type d'échec :

- le magnet a bien été récupéré mais l'**import** a échoué (erreur, import partiel, aucun fichier) :
  Retry relance l'import depuis le répertoire de staging, exactement comme pour qBittorrent ;
- **AllDebrid lui-même** a abandonné le magnet (erreur côté débrideur, donc avant toute récupération
  de fichier) : Retry relance le magnet (`POST /alldebrid/downloads/{id}/restart`) et la ligne repasse
  en *En cours* ;
- le magnet est **stalled** (aucun octet après 30 min) mais AllDebrid le considère encore actif : Retry
  réarme simplement la fenêtre de surveillance (voir *Magnet sans pair*).

:::note[Détails techniques]
**Formats récupérés** — `cbz`, `cbr`, `pdf`, `epub`. Un magnet qui ne contient aucun fichier de ces
extensions n'importe rien (statut *Aucun fichier importé*).

**Boucle de suivi unique** — contrairement à qBittorrent (un slot par téléchargement, parallélisé),
AllDebrid est interrogé par une seule sonde séquentielle : 5 s quand un magnet progresse, 15 s au
repos. AllDebrid limite le nombre de magnets actifs simultanés (30) et le débit de l'API (12 req/s) ;
c'est le débrideur qui assure le téléchargement, pas Stripstream.

**Codes de statut AllDebrid** — `0` à `3` en téléchargement, `4` terminé, `≥ 5` erreur.

**Récupération des fichiers** — le téléchargement des fichiers depuis AllDebrid est retenté jusqu'à
3 fois avant de marquer l'entrée en erreur, pour qu'une coupure réseau passagère ne condamne pas un
magnet terminé.

**Doublons** — un magnet déjà suivi par AllDebrid (même hash) est réutilisé au lieu d'être envoyé
une seconde fois.

**Endpoints admin** (jetons `admin`, documentés dans l'OpenAPI administrateur `/admin/openapi.json`) :

| Méthode | Endpoint | Rôle |
|---------|----------|------|
| `POST` | `/alldebrid/add` | Envoyer un magnet ou un fichier `.torrent` |
| `GET` | `/alldebrid/test` | Tester la clé API (compte premium) |
| `POST` | `/alldebrid/downloads/{id}/restart` | Relancer un magnet en erreur |

Rien n'est ajouté à l'API publique : AllDebrid reste un réglage d'administration, les clients de
lecture ne voient pas la clé et ne peuvent pas déclencher d'envoi.
:::
