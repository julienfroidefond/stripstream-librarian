---
title: qBittorrent
description: Téléchargement et import automatique via qBittorrent
---

## Ajout de torrents

Ajoutez des torrents directement depuis les résultats Prowlarr ou les downloads disponibles.

- **Mode replace** : importer tous les volumes d'un torrent (bypass du filtre expected_volumes)
- **Détection de doublons** : si le torrent existe déjà dans qBittorrent, détection par hash magnet, lecture du content_path réel, lancement immédiat de l'import si complété
- Endpoint de test de connexion

## Pipeline d'import

1. Le poller qBittorrent détecte les torrents complétés (timeout 5 min pour les hashes non résolus)
2. Extraction des volumes depuis les noms de fichiers (supporte `Tome_01` avec underscore)
3. Matching de série via `LOWER(unaccent())` (insensible casse + accents)
4. Nommage des fichiers depuis le livre de référence existant (conserve la convention)
5. Déduplication par format (cbz > cbr > pdf > epub)
6. **One-shot** : fichiers sans numéro de volume importés tels quels
7. **Fichiers existants** : comptés comme importés (pas d'erreur)
8. Rapport détaillé : fichiers importés, fichiers skippés avec raisons
9. Statut : `imported` ou `no_files_imported`
10. **Retry** : relancer les imports bloqués (vérifie que les fichiers source existent)
11. Cleanup : suppression du torrent + répertoire de téléchargement
12. Post-import : job de scan, refresh metadata si lié, mise à jour `available_downloads`
