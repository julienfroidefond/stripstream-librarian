---
title: Téléchargements — Vue d'ensemble
description: Quatre modes pour compléter votre bibliothèque — Prowlarr (qBittorrent ou AllDebrid), Telegram Monitor et fournisseurs directs
---

Stripstream propose quatre façons de télécharger des volumes manquants — trois sources et, pour les torrents, le choix du client de téléchargement :

| Mode | Prérequis | Fonctionnement |
|------|-----------|----------------|
| [Prowlarr + qBittorrent](prowlarr) | Prowlarr + qBittorrent configurés | Recherche automatique de torrents, téléchargement via qBittorrent, import auto |
| [Prowlarr + AllDebrid](/integrations/alldebrid) | Prowlarr + compte AllDebrid premium | Même recherche, téléchargement débridé côté AllDebrid — aucun client BitTorrent local |
| [Telegram Monitor](telegram) | Compte Telegram + API ID/Hash | Surveillance de channels Telegram, téléchargement direct sans torrent |
| [Fournisseurs directs](ddl-providers) | Aucun (catalogues publics) | Recherche dans des catalogues externes (Fourtoutici…), import direct dans la bibliothèque |

Les quatre modes alimentent la page **Téléchargements** : les offres des **fournisseurs directs** y apparaissent comme releases (badge vert) et leurs téléchargements dans l'historique. Une recherche ciblée reste possible depuis la fenêtre *Actions → Téléchargement direct* d'une série ou d'un livre ; le fichier est alors importé puis indexé immédiatement.

## Activation

Les téléchargements sont désactivés par défaut. Pour les activer :

**Settings → Téléchargements → activer "Activer les téléchargements"**

Ce toggle contrôle la visibilité de la page Téléchargements, les boutons d'envoi au client de téléchargement et les options de détection automatique dans les bibliothèques.

---

## Page Téléchargements

### Historique des téléchargements

La liste du haut regroupe tous les téléchargements en cours ou terminés — Prowlarr, Telegram et fournisseurs directs confondus.

![Historique des téléchargements — entrées Telegram importées avec statut, numéro de tome, channel source et actions](/screenshots/downloads-history.png)

| Statut | Description |
|--------|-------------|
| **En attente** | En file d'attente Telegram (attend un slot libre) |
| **En cours** | Téléchargement en progression (barre + vitesse + ETA) |
| **Terminé** | Torrent terminé (barre à 100 %), en attente d'import |
| **Import en cours** | Fichiers en cours de copie dans la bibliothèque (barre à 100 % conservée) |
| **Importé** | Fichiers copiés avec succès |
| **Partiel** | Certains fichiers importés, d'autres ignorés |
| **Aucun fichier importé** | Aucun fichier ne correspondait aux volumes attendus |
| **Erreur** | Import ou téléchargement échoué |

**Filtres** : Tous · Actifs · Importés · Erreur

**Actions** : Retry (↺) pour relancer, Annuler pour interrompre, Supprimer pour effacer de l'historique.

### Disponibles au téléchargement

En bas de la page, la liste unifiée des releases Prowlarr, des fichiers Telegram **et** des offres des fournisseurs directs détectés mais pas encore téléchargés.

![Section Disponibles au téléchargement — liste unifiée Prowlarr et Telegram avec compteurs, manquants et seeders](/screenshots/downloads-available.png)

Un filtre source (Tous / Prowlarr / Telegram / DDL) permet de n'afficher qu'un seul type. Le tri par date, seeders, volumes manquants ou nom s'applique aux deux.

→ Détails sur les actions disponibles : [Prowlarr](prowlarr#disponibles-au-téléchargement) · [Telegram](telegram#fichiers-disponibles)

### Releases masquées

L'icône œil à côté du titre "Disponibles au téléchargement" ouvre le panneau **Releases masquées**, qui regroupe :
- Les releases Prowlarr blacklistées (bouton × sur une ligne Prowlarr)
- Les fichiers Telegram dismissés (bouton × sur une ligne Telegram)
- Les offres des fournisseurs directs blacklistées (bouton × sur une ligne DDL)

Cliquer sur **Réafficher** réactive l'élément dans la liste des volumes disponibles.

![Panneau Releases masquées — release Prowlarr blacklistée avec bouton Réafficher](/screenshots/downloads-releases-masquees.png)

---

## Ré-importer un tome déjà présent

Quand un téléchargement apporte un volume que la bibliothèque possède déjà — version de meilleure qualité, changement de format, nouveau torrent du même tome — Stripstream **remplace** l'ancien fichier au lieu de créer un doublon :

- le fichier **le plus récent** (date de modification) est conservé ;
- l'ancien fichier est **supprimé physiquement** du disque ;
- le livre existant et sa **progression de lecture** sont conservés — aucune nouvelle fiche n'est créée.

Ce comportement vaut pour **Prowlarr** (qBittorrent comme [AllDebrid](/integrations/alldebrid)), **Telegram Monitor** et les **fournisseurs directs**. Avec le [mode Replace](prowlarr#mode-replace), le plus récent gagne également, quel que soit le volume attendu.

:::note
Seuls les volumes **réguliers** participent au remplacement. Les hors-séries, intégrales et one-shots ne sont pas concernés.
:::
