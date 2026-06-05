---
title: Tâches Telegram Monitor
description: Synchronisation complète et incrémentale des channels Telegram pour détecter les nouvelles disponibilités
---

Deux tâches gèrent la surveillance Telegram, selon la profondeur de recherche souhaitée :

| Type | Ce qu'il fait |
|------|--------------|
| `telegram_sync` | Recherche active par série — requête Telegram par série de la bibliothèque |
| `telegram_sync_incremental` | Parcours chronologique — uniquement les nouveaux messages depuis la dernière synchro |

Les deux alimentent la liste **Livres disponibles Telegram** sur la page Téléchargements.

---

## Règle métier : quelles séries sont recherchées ?

Le job `telegram_sync` ne recherche **pas** toutes les séries de la bibliothèque. Pour être incluse, une série doit satisfaire deux conditions simultanément :

1. **Lien metadata approuvé** — un `external_metadata_links` avec `status = 'approved'` existe pour cette série
2. **Volumes manquants** — au moins un `external_book_metadata` avec `book_id IS NULL` (tome attendu mais non possédé)

C'est exactement la même règle que la détection de téléchargements Prowlarr : seules les séries qu'on cherche activement à compléter sont scrutées.

Le job `telegram_sync_incremental` n'applique **pas** ce filtre : il parcourt tous les messages récents sans distinction, car il s'agit d'un scan chronologique et non d'une recherche ciblée.

---

## telegram_sync — synchronisation complète

### Déclenchement

**Manuel uniquement** — bouton *Sync complet* dans la page Tâches ou via **Settings → Telegram Monitor**. Il n'y a pas de planification automatique pour ce job.

### Ce que fait le job

```
Pour chaque channel activé avec bibliothèque configurée :
    Filtrer les séries éligibles (metadata approuvée + volumes manquants)
    Si aucune série éligible → passer au channel suivant
    Résoudre le username → chat Telegram
    Pour chaque série éligible :
        → search_messages(query = nom de la série, filtre = documents)
        → Pour chaque CBZ/CBR/PDF/EPUB/ZIP trouvé :
            Extraire le nom de série et le numéro de volume
            INSERT INTO telegram_book_links … ON CONFLICT DO NOTHING
    Mettre à jour le channel_title
Sauvegarder la session Telegram mise à jour
```

### Progression

| Champ | Valeur |
|-------|--------|
| `total_files` | Nombre total de séries à rechercher (toutes sources) |
| `processed_files` | Séries traitées |
| `current_file` | Label `@channel: nom de la série` en cours |

### Rapport final

| Champ | Description |
|-------|-------------|
| **Séries recherchées** | Nombre de requêtes effectuées sur Telegram |
| **Nouveaux livres** | Fichiers insérés pour la première fois |
| **Séries avec résultats** | Séries ayant retourné au moins un fichier, avec le nombre de documents et les noms extraits |
| **Séries liées** | Parmi les résultats, celles qui correspondent à une série existante dans la bibliothèque |

---

## telegram_sync_incremental — synchronisation incrémentale

### Déclenchement

**Automatique** : toutes les **30 minutes** dès que vous êtes authentifié. Aucune configuration nécessaire.

**Manuel** : bouton *Synchro incrémentale* dans la page Tâches.

### Ce que fait le job

```
Pour chaque channel activé :
    Récupérer MAX(message_id) déjà connu pour ce channel dans telegram_book_links
    Résoudre le username → chat Telegram
    Parcourir iter_messages (du plus récent vers le plus ancien)
    Dès qu'un message_id ≤ MAX connu → arrêter (déjà traité)
    Pour chaque CBZ/CBR/PDF/EPUB/ZIP trouvé :
        INSERT INTO telegram_book_links … ON CONFLICT DO NOTHING
Sauvegarder la session Telegram mise à jour
```

Lors du **premier run** (aucun message en base pour un channel), le job parcourt l'intégralité de l'historique du channel.

### Progression

| Champ | Valeur |
|-------|--------|
| `total_files` | Nombre de channels (sources) à traiter |
| `processed_files` | Channels traités |
| `current_file` | Label `@channel (incremental)` en cours |

### Rapport final

| Champ | Description |
|-------|-------------|
| **Sources analysées** | Nombre de channels parcourus |
| **Nouveaux livres** | Fichiers insérés pour la première fois |

---

## Livres disponibles après les jobs

À l'issue d'un sync (complet ou incrémental), les fichiers dont la série existe dans la bibliothèque apparaissent dans **Livres disponibles Telegram** sur la page Téléchargements. L'affichage est filtré dynamiquement : seules les séries avec un lien metadata approuvé et des volumes manquants sont présentées.

Voir [Telegram Monitor](/integrations/telegram-monitor/#livres-disponibles).

---

:::note[Détails techniques]
**Types de job** : `telegram_sync` et `telegram_sync_incremental`, tous deux avec `library_id = NULL` (globaux).

**Exécutés par** : le poller de l'API.

**Planifié par** : le scheduler de l'indexer (pour `telegram_sync_incremental` uniquement). Conditions :
1. `session_data` présent dans `app_settings` (compte authentifié)
2. Aucun job du même type en `pending` ou `running`
3. Aucun job du même type `finished_at > NOW() - INTERVAL '30 minutes'`

**Champs `stats_json` — `telegram_sync`** : `synced`, `new_books`, `series_searched`, `all_series` (tableau : `series_name`, `book_count`, `extracted_names`), `matched_series` (tableau : `telegram_name`, `series_id`, `series_name`, `book_count`).

**Champs `stats_json` — `telegram_sync_incremental`** : `new_books`, `sources_scanned`.

**Session** : sauvegardée en base après chaque sync (clé `session_data` dans `app_settings`). La session MTProto évolue à chaque connexion.

**Stale jobs** : les jobs `pending` depuis > 30 min sont marqués `failed` par le cleanup du scheduler.

**Contrainte DB** : le type `telegram_sync_incremental` est autorisé par la migration `0099_add_telegram_sync_incremental_job_type.sql`.
:::
