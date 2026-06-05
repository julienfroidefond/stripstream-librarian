---
title: Tâche Telegram Monitor
description: Synchronisation périodique des channels Telegram pour détecter les nouvelles disponibilités
---

La tâche `telegram_sync` parcourt chaque channel Telegram configuré et y recherche les fichiers correspondant aux séries de votre bibliothèque. Elle alimente la liste **Livres disponibles Telegram** sur la page Téléchargements.

---

## Déclenchement

**Automatique** : l'indexer crée un job `telegram_sync` selon l'intervalle configuré dans **Settings → Telegram Monitor → Intervalle de sync**. Mettre `0` désactive la planification automatique.

**Manuel** : bouton *Synchroniser* dans **Settings → Telegram Monitor** — déclenche un job immédiatement.

Un seul job `telegram_sync` est actif à la fois. Si un job est déjà en cours (`pending` ou `running`), un nouveau déclenchement est ignoré silencieusement.

---

## Ce que fait le job

```
Pour chaque channel activé :
    Résoudre le username → chat Telegram
    Pour chaque série de la bibliothèque associée :
        → search_messages(query = nom de la série, filtre = documents)
        → Pour chaque fichier CBZ/CBR/PDF/EPUB/ZIP trouvé :
            Extraire le nom de série et le numéro de volume
            INSERT INTO telegram_book_links … ON CONFLICT DO NOTHING
    Mettre à jour le channel_title
Sauvegarder la session Telegram mise à jour
```

Les fichiers déjà connus (même `source_id` + `message_id`) ne sont pas réinsérés.

---

## Rapport

Le détail du job affiche :

| Champ | Description |
|-------|-------------|
| **Messages scannés** | Total de documents parcourus sur tous les channels |
| **Nouveaux livres** | Fichiers insérés pour la première fois |
| **Séries recherchées** | Nombre de requêtes effectuées sur Telegram |
| **Séries avec résultats** | Séries ayant retourné au moins un fichier, avec le nombre de documents et les noms extraits |
| **Séries liées** | Parmi les résultats, celles qui correspondent à une série existante dans la bibliothèque |

---

## Conditions pour qu'un channel soit traité

- Le channel doit être **activé** dans Settings → Telegram Monitor
- Une **bibliothèque cible** doit être associée au channel — sans elle, aucune série à chercher
- Vous devez être **membre** du channel Telegram

---

## Livres disponibles après le job

À l'issue du job, les fichiers détectés dont la série existe dans la bibliothèque associée apparaissent dans la section **Livres disponibles Telegram** de la page Téléchargements. Voir [Telegram Monitor](/integrations/telegram-monitor/#livres-disponibles).

---

:::note[Détails techniques]
**Type de job** : `telegram_sync`, `library_id = NULL` (global).

**Exécuté par** : le poller de l'API (non-exclusif avec les autres types de jobs globaux comme `prowlarr_rss`).

**Planifié par** : le scheduler de l'indexer. Conditions pour créer un job :
1. `session_data` présent dans `app_settings` (compte authentifié)
2. `sync_interval_minutes > 0`
3. Aucun job `pending` ou `running` du même type
4. Aucun job `finished_at > NOW() - INTERVAL '{interval} minutes'`

**Champs `stats_json`** : `synced` (messages), `new_books`, `series_searched`, `all_series` (tableau par série : `series_name`, `book_count`, `extracted_names`), `matched_series` (tableau : `telegram_name`, `series_id`, `series_name`, `book_count`).

**Session** : sauvegardée en base après chaque sync (clé `session_data` dans `app_settings`). La session MTProto évolue à chaque connexion — ne pas sauter ce save.

**Stale jobs** : les jobs `pending` depuis > 30 min sont marqués `failed` par le cleanup du scheduler, libérant la place pour un prochain job planifié.
:::
