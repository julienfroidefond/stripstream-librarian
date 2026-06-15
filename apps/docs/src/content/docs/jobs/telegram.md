---
title: Tâches Telegram Monitor
description: Synchronisation complète et incrémentale des channels Telegram pour détecter les nouvelles disponibilités
---

Deux tâches gèrent la surveillance Telegram, selon la profondeur de recherche souhaitée :

| Type | Ce qu'il fait |
|------|--------------|
| Synchronisation complète | Recherche active par série dans vos channels |
| Synchronisation incrémentale | Parcours chronologique des nouveaux messages |

Les deux alimentent la section **Disponibles au téléchargement** de la page Téléchargements.

---

## Quelles séries sont recherchées ?

La synchronisation complète ne recherche **pas** toutes les séries de la bibliothèque. Pour être incluse, une série doit satisfaire deux conditions simultanément :

1. **Métadonnées validées** — Stripstream doit connaître la liste officielle des volumes.
2. **Volumes manquants** — au moins un volume connu doit être absent de votre bibliothèque.

Les séries qui possèdent une intégrale sont considérées complètes et exclues de cette recherche active.

C'est la même condition que la détection Prowlarr : seules les séries qu'on cherche activement à compléter sont scrutées.

La synchronisation incrémentale n'applique **pas** ce filtre : elle parcourt tous les messages récents sans distinction, car il s'agit d'un scan chronologique et non d'une recherche ciblée.

---

## Synchronisation complète

### Déclenchement

**Manuel uniquement** — bouton *Sync complet* dans la page Tâches ou via **Settings → Telegram Monitor**. Il n'y a pas de planification automatique.

### Ce qu'elle fait

Pour chaque channel activé, Stripstream cherche les séries éligibles une par une. Les fichiers compatibles trouvés sont ajoutés à la liste **Livres disponibles Telegram**.

### Progression

La progression indique le nombre de séries recherchées et la série en cours.

### Rapport final

| Champ | Description |
|-------|-------------|
| **Séries recherchées** | Nombre de requêtes effectuées sur Telegram |
| **Nouveaux livres** | Fichiers insérés pour la première fois |
| **Séries avec résultats** | Séries ayant retourné au moins un fichier, avec le nombre de documents et les noms extraits |
| **Séries liées** | Parmi les résultats, celles qui correspondent à une série existante dans la bibliothèque |

---

## Synchronisation incrémentale

### Déclenchement

**Automatique** : selon la période configurée dans **Settings → Telegram Monitor**. Par défaut, toutes les **30 minutes** dès que vous êtes authentifié.

**Manuel** : bouton *Synchro incrémentale* dans la page Tâches.

### Ce qu'elle fait

Pour chaque channel activé, Stripstream lit les messages récents jusqu'à retrouver un message déjà connu. Les nouveaux fichiers compatibles sont ajoutés à la liste **Livres disponibles Telegram**.

Lors du **premier run** (aucun message en base pour un channel), le job parcourt l'intégralité de l'historique du channel.

### Progression

La progression indique le nombre de channels analysés et le channel en cours.

### Rapport final

| Champ | Description |
|-------|-------------|
| **Sources analysées** | Nombre de channels parcourus |
| **Nouveaux livres** | Fichiers insérés pour la première fois |

---

## Résultat après les jobs

À l'issue d'un sync (complet ou incrémental), les fichiers dont la série existe dans la bibliothèque apparaissent dans la section **Disponibles au téléchargement** de la page Téléchargements, filtrables par source Telegram.

Voir [Téléchargements — Telegram](/downloads/telegram).

---

:::note[Détails techniques]
**Types de job** : `telegram_sync` et `telegram_sync_incremental`, tous deux avec `library_id = NULL` (globaux).

**Exécutés par** : le poller de l'API.

**Planifié par** : le scheduler de l'indexer (pour `telegram_sync_incremental` uniquement). Conditions :
1. `session_data` présent dans `app_settings` (compte authentifié)
2. Aucun job du même type en `pending` ou `running`
3. Aucun job du même type terminé récemment, selon l'intervalle configuré

**Champs `stats_json` — `telegram_sync`** : `synced`, `new_books`, `series_searched`, `all_series` (tableau : `series_name`, `book_count`, `extracted_names`), `matched_series` (tableau : `telegram_name`, `series_id`, `series_name`, `book_count`).

**Champs `stats_json` — `telegram_sync_incremental`** : `new_books`, `sources_scanned`.

**Session** : sauvegardée en base après chaque sync (clé `session_data` dans `app_settings`). La session MTProto évolue à chaque connexion.

**Stale jobs** : les jobs `pending` depuis > 30 min sont marqués `failed` par le cleanup du scheduler.

**Contrainte DB** : le type `telegram_sync_incremental` est autorisé par la migration `0099_add_telegram_sync_incremental_job_type.sql`.

**Règle d'éligibilité complète** : `telegram_sync` requiert un `external_metadata_links` avec `status = 'approved'` et au moins un `external_book_metadata` avec `book_id IS NULL`. Les séries avec `volume_type = 'integral'` sont exclues.

**Pipeline `telegram_sync`** :
```
Pour chaque channel activé avec bibliothèque configurée :
    Filtrer les séries éligibles
    Résoudre le username → chat Telegram
    Pour chaque série éligible :
        → search_messages(query = nom de la série, filtre = documents)
        → Pour chaque fichier compatible trouvé :
            INSERT INTO telegram_book_links … ON CONFLICT DO NOTHING
    Mettre à jour le channel_title
Sauvegarder la session Telegram mise à jour
```

**Pipeline `telegram_sync_incremental`** :
```
Pour chaque channel activé :
    Récupérer MAX(message_id) déjà connu pour ce channel dans telegram_book_links
    Résoudre le username → chat Telegram
    Parcourir iter_messages du plus récent vers le plus ancien
    Dès qu'un message_id ≤ MAX connu → arrêter
    Pour chaque fichier compatible trouvé :
        INSERT INTO telegram_book_links … ON CONFLICT DO NOTHING
Sauvegarder la session Telegram mise à jour
```
:::
