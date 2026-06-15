---
title: Téléchargements — Telegram
description: Télécharger des livres depuis des channels Telegram directement dans votre bibliothèque
---

Telegram Monitor surveille des channels Telegram et met à disposition les fichiers détectés directement dans la page Téléchargements, sans passer par un client torrent.

Pour la configuration (API ID, authentification, channels, synchronisation), voir [Telegram Monitor](/integrations/telegram-monitor).

## Fichiers disponibles

Les fichiers Telegram détectés dont la série est reconnue dans votre bibliothèque apparaissent dans la section **Disponibles au téléchargement**, mêlés aux releases Prowlarr. Utilisez le filtre **Telegram** pour n'afficher que les fichiers Telegram.

Chaque ligne Telegram dans la liste dépliée affiche :
- Badge **TG** (bleu) + numéro de tome si détecté (vert) ou `?` si non reconnu
- Nom du fichier complet
- Channel source (`@channel`)
- Taille du fichier
- Actions (voir ci-dessous)

### Actions sur un fichier

| Bouton | Effet |
|--------|-------|
| **↓ (bleu)** | Lance le téléchargement — le fichier passe en file d'attente |
| **Icône horloge** | Fichier en attente ou en cours — pas d'action disponible |
| **↺ (retry, rouge)** | Relancer le téléchargement d'un fichier en erreur |
| **× (neutre)** | *Dismiss* réversible — masque le fichier et l'ajoute aux *Releases masquées* |
| **Poubelle** | Suppression définitive — retire l'entrée de la base de données |

Le bouton × est réversible : supprimer l'entrée depuis le panneau *Releases masquées* (icône œil) restaure le fichier en *Disponible*.

## Flux de téléchargement

1. Clic sur **↓** — le fichier entre dans la file de téléchargement (statut *En attente*)
2. Dès qu'un slot est libre, Stripstream se connecte à Telegram et récupère le fichier
3. Le fichier est téléchargé en streaming dans le répertoire de la série (créé si inexistant)
4. Un scan est déclenché automatiquement pour intégrer le nouveau fichier
5. Statut final : **Importé** ou **Erreur**

Le nombre de téléchargements simultanés est configurable dans Settings → Telegram Monitor (défaut : 2). Un timeout de 30 minutes s'applique par fichier.

## Nommage à l'import

Le fichier téléchargé est renommé selon le [template de renommage](/books/renaming/#renommage-automatique-à-limport) quand la série n'a pas encore de livre. Par exemple, avec `{series_name} - T{volume_padded}`, `Frieren Tome 3 - Le voyage.cbz` devient `Frieren - T03.cbz`.

Si la série a déjà des livres, le nommage existant est repris pour garder une collection homogène.

## Suivi dans l'historique

Les téléchargements Telegram apparaissent dans l'historique en haut de la page, mêlés aux torrents qBittorrent.

| Statut | Description |
|--------|-------------|
| **En attente** | En file d'attente (icône horloge) |
| **Téléchargement** | En cours depuis Telegram (barre de progression) |
| **Importé** | Fichier copié dans la bibliothèque |
| **Erreur** | Échec du téléchargement ou de la copie |

**Filtres** : Actifs (inclut En attente + Téléchargement), Importés, Erreur.

**Actions dans l'historique** :
- **↺ Retry** — relancer un téléchargement en erreur ou re-télécharger un fichier déjà importé (utile si le fichier a été supprimé manuellement)
- **Annuler / Supprimer** — annuler un téléchargement en attente ou en cours (libère le slot immédiatement), ou retirer une entrée terminée

## Recherche depuis une série

Sur la fiche d'une série → *Actions* → **Rechercher sur Telegram** : recherche live sur tous vos channels configurés, quel que soit le statut metadata de la série.

Dans la fenêtre de recherche :
- Champ de recherche pré-rempli avec le nom de la série (modifiable)
- Badges tomes manquants — raccourcis pour chercher un tome précis
- Résultats avec statut et actions par ligne

| Statut dans la fenêtre | Action |
|------------------------|--------|
| Disponible | Télécharger + Ignorer |
| En attente | Icône horloge |
| Téléchargement | Barre de progression |
| Importé | ↺ pour re-télécharger |
| Ignoré | Aucune (ligne grisée) |

Les résultats restent affichés après action et leur statut se met à jour sans fermer la fenêtre.

:::note[Détails techniques]
**Répertoire de destination** : résolu dans cet ordre — (1) fichier existant de la série en DB, (2) répertoire existant dans la bibliothèque dont le nom correspond, (3) nouveau répertoire `bibliothèque/nom-de-série`.

**Dismiss (×)** : `status='dismissed'` + insertion dans `release_blacklist`. Cliquer sur **Réafficher** dans le panneau *Releases masquées* restaure `status='available'` via une CTE atomique.

**Hard delete (poubelle)** : `DELETE FROM telegram_book_links ?hard=true` — suppression définitive.

**Réimport** : les entrées `imported` sont réconciliées à l'affichage. Si le livre correspondant n'existe plus en base, le statut repasse automatiquement à `available`.

**Reset au démarrage** : les téléchargements bloqués en `queued` ou `downloading` au redémarrage sont réinitialisés à `available`.
:::
