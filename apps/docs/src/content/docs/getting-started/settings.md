---
title: Paramètres
description: Vue d'ensemble des réglages du backoffice — les neuf onglets et ce qu'ils contrôlent
---

La page **Paramètres** (`/settings`) rassemble la configuration de l'instance : performances,
images et cache, téléchargements, métadonnées, IAG, statut de lecture, notifications, jetons et
archives. Elle est accessible depuis le menu principal (icône engrenage).

Les réglages sont appliqués **à chaud** : aucun redémarrage des services n'est nécessaire. Ils
sont aussi lisibles et modifiables via l'API (`GET` / `POST /settings/{clé}`, réservés aux tokens
`admin`), ce qui permet de scripter la configuration.

## Les neuf onglets

| Onglet | Ce qu'il contrôle |
|--------|-------------------|
| **Général** | Langue de l'interface, limites de performance, format de renommage, versions |
| **Images & cache** | Rendu des pages, cache disque/mémoire, réglages des miniatures |
| **Outils de téléchargement** | Prowlarr, qBittorrent, AllDebrid, moteur DDL, fournisseurs directs, Telegram Monitor |
| **Métadonnées** | Fournisseurs de métadonnées (clés API) et correspondance de statuts |
| **IAG** | Endpoint, modèle et prompts de l'IA (genres et listes de lecture) |
| **Statut de lecture** | Synchronisation AniList et import Komga |
| **Notifications** | Bot Telegram et événements notifiés |
| **Jetons** | Utilisateurs lecteurs et tokens d'API |
| **Archives** | Séries supprimées du disque conservées avec leur historique |

---

## Général

- **Langue** — français ou anglais ; le choix est mémorisé et persiste entre les sessions.
- **Limites de performance** — `concurrent_renders` (nombre de pages rendues simultanément),
  `timeout_seconds` (délai d'expiration d'un rendu) et `rate_limit_per_second` (limite de débit de
  l'API de lecture). Ces trois limites sont **globales** (clé `limits`).
- **Format de renommage des livres** — modèles de nommage par type de tome ; voir
  [Renommage](/books/renaming/).
- **À propos** — versions des services **API**, **Indexer** et **Backoffice**. Comparer ces
  versions révèle un conteneur non redéployé ; le [diagnostic](/getting-started/diagnostics/) fait
  de même pour les battements de cœur.

## Images & cache

Trois cartes :

- **Traitement d'images** — format de sortie par défaut (WebP / JPEG / PNG), qualité (1–100),
  filtre de redimensionnement (lanczos3, triangle, nearest) et largeur maximale autorisée. Ces
  valeurs s'appliquent au **rendu des pages** servi aux lecteurs, pas aux miniatures.
- **Cache** — statistiques du cache de pages (taille disque, fichiers, cache RAM occupé sur le
  plafond, pages en mémoire, répertoire), puis réglages : **répertoire du cache**, **taille max
  (Mo)** sur disque et **cache pages en RAM (Mo)**. Le bouton **Vider le cache** purge mémoire et
  disque. Détails dans [Miniatures](/books/thumbnails/).
- **Miniatures** — activation, format de sortie (original, WebP, JPEG, PNG), largeur, hauteur,
  qualité, répertoire, plus les compteurs de taille et de fichiers. La génération est assurée par
  l'**Indexeur** ; voir [Miniatures](/books/thumbnails/).

:::caution
Un format de miniature **original** conserve la première page telle quelle (pas de réencodage) :
plus fidèle, mais potentiellement beaucoup plus volumineux. Changez le répertoire du cache ou des
miniatures seulement vers un chemin monté et accessible par le conteneur.
:::

## Outils de téléchargement

Un **interrupteur global** active ou désactive toutes les fonctionnalités de téléchargement. Tant
qu'il est désactivé, l'interface de recherche, le client de téléchargement et les jobs de détection
restent inertes.

Les sections suivantes :

- **Provider** — configuration [Prowlarr](/integrations/prowlarr/).
- **Clients de téléchargement** — le **client de téléchargement** reçoit *tous* les snatchs, jamais
  les deux : [qBittorrent](/integrations/qbittorrent/) ou [AllDebrid](/integrations/alldebrid/).
  Un client désactivé ou non configuré apparaît grisé.
- **Moteur de téléchargement** — limites des transferts directs (DDL).
- **DDL** — fournisseurs directs ([Téléchargement direct](/downloads/ddl-providers/)).
- **Telegram** — [Telegram Monitor](/integrations/telegram-monitor/) : API ID, authentification,
  channels et fréquence de synchronisation incrémentale.

Voir aussi la [vue d'ensemble des téléchargements](/downloads/overview/).

## Métadonnées

- **Fournisseurs de métadonnées** — clés API et activation par provider (ComicVine, Google Books,
  Open Library… ; SensCritique, AniList, BDTheque et BDphile ne nécessitent pas de clé). Voir
  [Providers](/metadata/providers/).
- **Correspondance de statuts** — normalisation des statuts renvoyés par les providers
  (`ongoing`, `ended`…) vers vos propres libellés. Voir [Providers](/metadata/providers/).

## IAG

Configuration de l'IA de suggestion de genres et de listes de lecture : endpoint OpenAI-compatible,
clé API, modèle, nombre maximum de tags et prompts. Deux interrupteurs indépendants activent la
proposition de genres et la proposition de listes de lecture ; les deux fonctionnalités partagent
l'endpoint, la clé et le modèle. Voir [Genres](/series/genres/) et
[Listes de lecture](/series/reading-lists/).

## Statut de lecture

- **AniList** — liaison du compte et synchronisation bidirectionnelle de la progression ; voir
  [AniList](/integrations/anilist/).
- **Komga** — import unidirectionnel des livres lus ; voir [Komga](/integrations/komga/).

## Notifications

Bot Telegram et choix des événements notifiés ; voir [Telegram — Notifications](/integrations/telegram/).

## Jetons

Utilisateurs lecteurs et tokens d'API ; voir [Utilisateurs & Tokens](/reference/users-tokens/).

## Archives

Séries supprimées du disque mais conservées avec leur historique de lecture ; voir
[Archives](/series/archives/).

---

:::note[Détails techniques]
**Lecture / écriture** : `GET /settings/{clé}` et `POST /settings/{clé}` sont des routes
génériques — chaque clé est un réglage nommé (`image_processing`, `cache`, `limits`, `thumbnail`,
`ai_tagging`, mais aussi `prowlarr`, `qbittorrent`, `alldebrid`, `torrent_import`, `telegram`,
`anilist`, `komga`, `metadata_providers`, `status-mappings`, `download_client`,
`downloads_enabled`, `rename_format`…).
Il n'existe pas de handler distinct par intégration.

**Validation** : `POST /settings/{key}` rejette les valeurs invalides (`400 Bad Request`) — voir
[API](/reference/api/#authentification) pour les règles sur `limits` et `cache`.

**Secrets** : les clés API et mots de passe ne sont jamais renvoyés en clair par les endpoints de
lecture (`GET /settings/alldebrid` les masque, par exemple).
:::
