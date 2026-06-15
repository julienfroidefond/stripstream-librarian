---
title: Téléchargements — Prowlarr
description: Trouver et télécharger automatiquement des volumes manquants via Prowlarr et qBittorrent
---

Ce mode automatise la recherche de torrents via Prowlarr et leur téléchargement via qBittorrent, puis importe les fichiers dans votre bibliothèque.

## Prérequis

| Intégration | Ce qu'il faut renseigner |
|-------------|--------------------------|
| [Prowlarr](/integrations/prowlarr) | URL + clé API |
| [qBittorrent](/integrations/qbittorrent) | URL + nom d'utilisateur + mot de passe |

## Détection des volumes manquants

Stripstream cherche des releases dans Prowlarr pour les séries qui remplissent **les deux conditions** :

1. **Métadonnées validées** — un lien metadata approuvé avec la liste officielle des volumes.
2. **Volumes manquants** — au moins un tome référencé absent de votre bibliothèque.

Une série sans metadata, complète, ou couverte par une intégrale est ignorée.

**Déclenchement** :
- **Automatique** : configurez une fréquence dans les paramètres de chaque bibliothèque → section *Détection de téléchargements*
- **Manuel** : depuis la fiche d'une série, bouton **Prowlarr** → recherche ciblée

## Disponibles au téléchargement

Les releases trouvées apparaissent dans la section **Disponibles au téléchargement** de la page Téléchargements (partagée avec Telegram Monitor).

Pour chaque release Prowlarr dans la liste dépliée :
- Volumes couverts (badges verts)
- Nom de la release (fonte monospace)
- Indexeur source
- Nombre de seeders (vert ≥ 10, orange ≥ 3, rouge sinon)
- Taille
- **Bouton ↓ (bleu)** — envoie la release à qBittorrent
- **Bouton × (neutre)** — blacklist : masque définitivement cette release dans les *Releases masquées*
- **Bouton poubelle** — supprime la release sans blacklister

Le badge `!` sur une ligne signale une release qui a déjà échoué à l'import.

**Masquer ces résultats** : supprime toutes les releases Prowlarr d'une série sans les blacklister (lien discret en bas du groupe déplié).

## Envoi à qBittorrent

Quand vous cliquez sur ↓, Stripstream envoie le torrent à qBittorrent. La release passe dans l'historique en haut de la page avec suivi en temps réel (progression, vitesse, ETA).

### Mode Replace

L'option **Replace** importe tous les fichiers du torrent sans se limiter aux volumes manquants attendus. Utile pour remplacer des fichiers existants par une version de meilleure qualité.

## Import automatique

Quand qBittorrent termine, Stripstream importe les fichiers dans la bibliothèque. Le nom du fichier final suit ces règles :

1. Si la série a déjà des livres, le nommage du fichier existant le plus pertinent est repris.
2. Sinon, le [template de renommage](/books/renaming/#renommage-automatique-à-limport) est appliqué (ex. `{series_name} - T{volume_padded}`).
3. Si le template ne s'applique pas, le nom source est conservé après nettoyage.

Après l'import, un scan + refresh des métadonnées est déclenché automatiquement.

:::note[Détails techniques]
La détection repose sur un lien metadata approuvé et des volumes externes non associés à un livre local. Une série couverte par `volume_type = 'integral'` est considérée complète.

Le padding de `{volume_padded}` utilise `series.total_volumes` si disponible, sinon les volumes détectés dans le téléchargement.

Le mode replace ignore la liste `expected_volumes` au moment de l'import et importe tous les fichiers compatibles.
:::

## Statuts de l'historique

| Statut | Description |
|--------|-------------|
| **En cours** | Téléchargement en progression dans qBittorrent |
| **Terminé** | Torrent terminé, en attente d'import |
| **Import en cours** | Fichiers en cours de copie |
| **Importé** | Fichiers copiés, scan lancé |
| **Partiel** | Certains fichiers importés, d'autres ignorés |
| **Aucun fichier importé** | Aucun fichier ne correspondait aux volumes attendus |
| **Erreur** | Import échoué |

**Actions** :
- **Retry (↺)** — relancer l'import pour les statuts Erreur, Partiel, Aucun fichier importé
- **Annuler / Supprimer** — interrompre un téléchargement en cours ou retirer l'entrée

## Blacklist

Les releases blacklistées (bouton ×) n'apparaissent plus dans les prochaines détections. Pour les gérer : icône œil à côté de "Disponibles au téléchargement" → panneau *Releases masquées* → bouton **Réafficher**.
