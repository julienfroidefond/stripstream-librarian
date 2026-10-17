---
title: Diagnostic / Santé
description: Page de diagnostic du backoffice — état des services, de la base, des disques, des caches et des tâches
---

La page **Diagnostic** (menu principal, icône activité, route `/diagnostics`) rassemble en un seul
écran l'état de santé de l'installation : services, base de données, disques, caches, miniatures,
tâches d'indexation, intégrité des données et état par bibliothèque. Elle sert de premier réflexe
quand quelque chose semble anormal (lenteurs, miniatures manquantes, erreurs d'indexation, indexeur
arrêté).

## Indicateurs globaux

Huit compteurs en haut de page :

**Version API** · **En ligne depuis** (uptime du processus) · **Livres** · **Bibliothèques** ·
**Taille des fichiers** · **Tâches en cours** · **Tâches bloquées** · **Intégrité des données**

## Services

L'API et l'indexeur tournent dans deux conteneurs distincts et peuvent tomber indépendamment. Chacun
écrit un **battement de cœur** (`service_heartbeats`) toutes les 30 secondes ; la carte affiche pour
chaque service :

- **En ligne / Hors ligne** — *Hors ligne* au-delà de 90 secondes sans battement (un indexeur mort
  laisse sinon une file de tâches vide, indiscernable d'un système sain au repos)
- la **version** et, si elle diffère de celle de l'API, un badge **Versions différentes** (image de
  conteneur non redéployée d'un côté)
- l'**uptime** du processus et l'âge du dernier battement

## Base de données

- **Statut** — *Accessible* / *Injoignable*, résultat d'une requête de contrôle avec délai maximal de
  3 secondes
- **Latence** — temps de réponse de cette requête
- **Taille** et **migrations appliquées** (avec la dernière migration exécutée)
- **Connexions** — connexions actives sur le maximum autorisé par PostgreSQL

## Espace disque

Capacité des volumes utilisés par les services : **Bibliothèques**, **Miniatures** et **Cache
d'images**. Chaque volume affiche l'espace libre, la taille totale, une barre d'occupation (orange
au-delà de 80 %, rouge au-delà de 90 %) et son chemin. Un chemin inaccessible (volume non monté)
est signalé sans faire échouer la page — un disque plein est la panne la plus fréquente et la plus
silencieuse.

## Processus API

Mémoire résidente, **descripteurs de fichiers ouverts** (Linux uniquement, les conteneurs de
production) et **rendus en cours** sur le plafond de rendus simultanés (`concurrent_renders`). Un
plafond saturé explique une lecture qui stagne.

## Cache des pages

Le détail du cache utilisé pour servir les pages rendues (voir [Miniatures](/books/thumbnails) pour le
cycle de vie des images) :

- **Cache mémoire** — taille occupée sur le plafond configuré, nombre de pages en mémoire
- **Cache disque** — taille et nombre de fichiers
- **Taux de succès** — part des requêtes servies sans nouveau rendu
- **Répertoire** — dossier du cache

Le bouton **Purger le cache** vide le cache mémoire et le cache disque, puis rafraîchit les
statistiques affichées.

## Miniatures

Nombre de fichiers stockés, taille totale, **miniatures manquantes** et **livres non analysés**
(`page_count` encore inconnu, c'est-à-dire pas encore passés en phase d'analyse par l'indexeur).

## Tâches d'indexation

Total, en attente, en cours, échecs des dernières 24 h, **tâches bloquées** et répartition par type
(en attente, en cours, échecs sur 24 h, dernier passage).

Une tâche **bloquée** est une tâche `pending` depuis plus de 30 minutes : le planificateur la
considère comme en cours et n'en programme plus de nouvelle pour cette bibliothèque. La file se fige
silencieusement — c'est le premier chiffre à regarder si plus rien ne progresse. La carte liste
ensuite les **échecs récents** (type de tâche, bibliothèque, date, message d'erreur).

## Intégrité des données

Compteurs d'incohérences calculés sans accès disque :

- **Livres sans fichier** — aucune ligne dans `book_files`
- **Livres à 0 page** — analysés mais page count nul
- **Séries sans métadonnées** — aucune source externe appariée
- **Tomes en doublon** — plusieurs livres pour un même tome (`volume_type = 'regular'`)

## Sonde à la demande

Rien n'est exécuté au chargement de la page : la sonde **contacte des services externes** et
**parcourt les fichiers**, elle se lance donc manuellement via **Lancer la sonde** :

- **Intégrations** — qBittorrent, Prowlarr, Telegram et chaque fournisseur DDL activé : *Joignable*,
  *Injoignable* ou *Non configurée*, avec la latence et un détail court (code HTTP, `timeout`…). Les
  messages sont volontairement réduits à une classe d'erreur : ils ne contiennent ni URL ni jeton.
- **Fichiers de la bibliothèque** — chaque chemin de `book_files` (plafonné à 5 000) est testé sur
  disque pour détecter les fichiers déplacés ou supprimés, avec la liste des chemins manquants.

## Bibliothèques

Un tableau par bibliothèque : statut activé/désactivé, nombre de livres, livres non analysés,
miniatures manquantes, taille sur disque, et pour le **scan**, le **rafraîchissement des
métadonnées** et la **détection des téléchargements**, la date du dernier passage et celle du
prochain.

## Rafraîchissement, copie et erreurs

- Le bouton **Rafraîchir** en haut de page recharge toutes les données.
- Le bouton **Copier le diagnostic** copie le payload JSON complet dans le presse-papiers, à coller
  dans un rapport de bug. Le contenu est déjà assaini côté API.
- Si l'API ne répond pas, la page affiche un encart **API injoignable** avec le message d'erreur et un
  bouton **Réessayer** — aucune autre page n'est nécessaire.

## Endpoints sous-jacents

Toutes les données proviennent de deux endpoints réservés aux tokens `admin` :

| Endpoint | Contenu |
|----------|---------|
| `GET /diagnostics` | payload agrégé affiché au chargement de la page |
| `GET /diagnostics/probe` | sonde explicite : intégrations externes + existence des fichiers |

Ils sont documentés dans l'OpenAPI administrateur (`/admin/openapi.json`, UI `/admin/swagger-ui`)
sous le tag `diagnostics`. Les payloads ne contiennent aucune donnée sensible (pas de chaîne de
connexion, de jeton ni de secret) : uniquement des versions, tailles, compteurs et répertoires de
cache.
