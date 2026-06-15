---
title: Telegram Monitor
description: Surveiller des channels Telegram pour télécharger automatiquement des livres (CBZ, CBR, PDF, EPUB)
---

Telegram Monitor surveille des channels Telegram à la recherche de livres (CBZ, CBR, PDF, EPUB) partagés en tant que documents. Quand un fichier correspondant à une série de votre bibliothèque est détecté, vous pouvez le télécharger en un clic directement depuis Stripstream — sans client torrent ni indexeur externe.

## Prérequis

Telegram Monitor utilise l'API officielle Telegram (MTProto) et non un bot. Il vous faut :

- Un **compte Telegram** actif
- Un **API ID** et un **API Hash** — obtenez-les sur [my.telegram.org](https://my.telegram.org) → *API development tools*

:::note
Ces identifiants sont liés à votre compte Telegram personnel. Ils permettent à Stripstream d'agir comme un client Telegram (comme l'application officielle), pas comme un bot.
:::

---

## Configuration

Dans **Settings → Telegram Monitor** :

1. **API ID** — le numéro fourni par my.telegram.org
2. **API Hash** — la chaîne hexadécimale associée
3. **Numéro de téléphone** — votre numéro au format international (`+33612345678`)

Enregistrez, puis procédez à l'authentification.

---

## Authentification

L'authentification Telegram se fait en deux étapes :

1. **Envoyer le code** — cliquez sur le bouton *Envoyer le code*. Telegram envoie un code de vérification à votre application Telegram (ou par SMS si nécessaire).
2. **Vérifier** — saisissez le code reçu dans le champ *Code de vérification* et validez.

Une fois authentifié, la session est chiffrée et stockée en base de données. Vous n'avez pas à vous réauthentifier sauf si vous déconnectez explicitement votre session.

Le bouton **Déconnecter** efface la session stockée. Une nouvelle authentification sera nécessaire pour reprendre la surveillance.

---

## Channels surveillés

Une fois authentifié, la section **Channels surveillés** permet de gérer vos sources.

### Options de téléchargement

Deux paramètres s'affichent au-dessus de la liste de channels :

| Paramètre | Description |
|-----------|-------------|
| **Période de sync incrémentale** | Fréquence de la sync automatique (30 min, 1h, 1 jour, 1 mois) |
| **Téléchargements simultanés** | Nombre maximal de fichiers téléchargés en parallèle (défaut : 2) |

:::tip[FLOOD_WAIT]
Telegram applique une limite de débit sur les téléchargements simultanés. Si vous recevez des erreurs `FLOOD_WAIT`, réduisez le nombre de téléchargements simultanés. La modification prend effet après redémarrage du serveur.
:::

### Ajouter un channel

- **@username** — saisissez le nom d'utilisateur du channel Telegram (avec ou sans `@`)
- **Bibliothèque cible** — associez le channel à une bibliothèque. Les fichiers trouvés seront placés dans cette bibliothèque et les séries de cette bibliothèque seront utilisées pour les recherches.

:::caution
Vous devez être **membre** du channel Telegram pour que la synchronisation fonctionne. Rejoignez le channel depuis votre application Telegram avant de l'ajouter ici.
:::

---

## Synchronisation

Deux modes de synchronisation coexistent :

### Sync incrémentale (automatique, toutes les 30 min)

La sync incrémentale parcourt les **nouveaux messages** depuis la dernière exécution pour chaque channel. Elle traite tous les fichiers trouvés sans filtre sur les séries.

Elle tourne en arrière-plan toutes les 30 minutes dès que vous êtes authentifié — aucune configuration nécessaire.

### Sync complète (manuelle)

La sync complète effectue une **recherche active par série** sur chaque channel : elle interroge l'API Telegram avec le nom de chaque série éligible comme requête de recherche.

:::note[Quelles séries sont recherchées ?]
La sync complète ne recherche **pas** toutes les séries de votre bibliothèque. Pour être incluse, une série doit remplir deux conditions :

1. **Lien metadata approuvé** — un lien vers un provider externe (Anilist, Bedetheque…) avec statut *approuvé*
2. **Volumes manquants** — au moins un tome référencé dans les metadata mais absent de votre bibliothèque

Une série qui possède une **intégrale** est considérée complète et n'est pas recherchée automatiquement.

C'est intentionnel : seules les séries que vous cherchez activement à compléter sont scrutées, ce qui limite le nombre de requêtes Telegram.

Si une série n'a pas de metadata liée, utilisez la [recherche depuis la fiche série](#recherche-depuis-une-série) pour la trouver manuellement.
:::

Elle se déclenche via le bouton **Sync complet** dans Settings → Telegram Monitor ou dans la page Tâches.

---

## Extraction du nom de série et du numéro de volume

Stripstream analyse le nom de fichier pour en extraire le nom de la série et le numéro de tome. Les formats reconnus incluent :

| Exemple | Série | Tome |
|---------|-------|------|
| `One Piece - Tome 47.cbz` | One Piece | 47 |
| `Berserk - 32@BD_fr.cbz` | Berserk | 32 |
| `Naruto - Vol. 3.cbz` | Naruto | 3 |
| `Toriko T12.cbz` | Toriko | 12 |
| `Black_Clover_29_Titre@BD_fr.cbz` | Black Clover | 29 |
| `Détective Conan 02 (Auteur)@channel.cbz` | Détective Conan | 2 |

Le suffixe `@channel` ajouté par certains channels Telegram (`@BD_fr`, `@manga_fr`...) est ignoré lors de l'extraction.

---

## Livres disponibles

Les livres détectés dont la série est reconnue dans votre bibliothèque apparaissent dans la section **Livres disponibles Telegram** en bas de la page Téléchargements.

Les livres sont regroupés par série. Pour chaque groupe :
- Nom de la série (lien vers la fiche)
- Nombre de fichiers disponibles
- Tri par date, nombre ou nom

En dépliant un groupe, pour chaque fichier :
- Numéro de tome (si extrait)
- Nom du fichier complet
- Channel source (`@channel`)
- Taille du fichier
- Bouton **Télécharger** — lance le téléchargement direct
- Bouton **Ignorer** — masque définitivement ce fichier (status `dismissed`)

Après avoir cliqué sur **Télécharger**, le fichier passe au statut **En attente** et reste visible dans la liste (il ne disparaît pas). Le bouton de téléchargement est remplacé par une icône horloge le temps que le fichier soit pris en charge.

:::note
Seuls les livres dont la série existe dans votre bibliothèque sont affichés ici. Les livres non associés restent en base mais n'apparaissent pas dans cet écran — utilisez la [recherche depuis la fiche série](#recherche-depuis-une-série) pour les retrouver.
:::

---

## Recherche depuis une série

Sur la fiche d'une série, le menu *Actions* → section *Téléchargement* expose le bouton **Rechercher sur Telegram**.

Cette recherche est **live** : elle interroge directement l'API Telegram en temps réel sur tous vos channels configurés, puis affiche les résultats. Elle fonctionne pour **n'importe quelle série**, qu'elle ait ou non un lien metadata approuvé.

Dans la fenêtre de recherche :

- **Champ de recherche** — modifiable pour affiner la requête (le nom de la série est pré-rempli, sans l'article initial si présent)
- **Badge de la série** — relance la recherche avec le nom court ou complet
- **Badges des tomes manquants** — raccourcis pour chercher un tome précis
- **Résultats** — table scrollable avec : numéro de tome, nom de fichier complet, channel source, taille, statut, actions

Pour chaque résultat :

| Statut | Action disponible |
|--------|-------------------|
| **Disponible** | Bouton télécharger + bouton ignorer |
| **En attente** | Icône horloge (en file d'attente) |
| **Téléchargement** | Indicateur de progression |
| **Importé** | Bouton ↺ pour forcer le re-téléchargement |
| **Ignoré** | Aucune action (ligne grisée) |

Les résultats ne disparaissent pas au clic : leur statut est mis à jour dans la fenêtre sans la fermer.

---

## Téléchargement

Lorsque vous cliquez sur **Télécharger** (depuis la page Téléchargements ou depuis la recherche série) :

1. Le fichier passe au statut **En attente** — il entre dans la file de téléchargement
2. Dès qu'un slot est disponible (selon le paramètre *Téléchargements simultanés*), Stripstream se connecte à Telegram et récupère le message
3. Le fichier est téléchargé en streaming dans le répertoire de la série (détecté depuis la bibliothèque ou créé si inexistant)
4. Un job de scan est déclenché automatiquement pour intégrer le nouveau fichier
5. Le statut passe à **Téléchargement** (avec barre de progression), puis **Importé**

Le fichier téléchargé utilise le même [template de renommage des livres](/books/renaming/#renommage-automatique-à-limport) que les imports qBittorrent quand la série n'a pas encore de livre. Par exemple, avec `{series_name} - T{volume_padded}`, `Frieren Tome 3 - Le voyage.cbz` devient `Frieren - T03.cbz`.

Le téléchargement s'effectue en arrière-plan — vous pouvez continuer à utiliser Stripstream pendant ce temps. Un timeout de 30 minutes s'applique par fichier.

:::note[Forcer le re-téléchargement]
Un fichier Telegram déjà importé peut être re-téléchargé en cliquant sur le bouton **↺** (page Téléchargements ou fenêtre de recherche). Le fichier existant est d'abord supprimé, puis retéléchargé depuis Telegram. Utile si le fichier a été supprimé manuellement ou pour le remplacer par une meilleure version.

Alternativement, si vous supprimez le livre de la bibliothèque, Stripstream détecte automatiquement que le volume n'existe plus et repasse l'entrée en `Disponible`.
:::

---

## Suivi dans la page Téléchargements

Les téléchargements Telegram apparaissent dans l'historique en haut de la page Téléchargements, mêlés aux téléchargements qBittorrent.

**Tri** : les téléchargements actifs (en attente + en cours) apparaissent en premier, triés par date de lancement (le plus ancien en tête). Les téléchargements terminés suivent, du plus récent au plus ancien.

| Statut | Description |
|--------|-------------|
| **En attente** | En file d'attente, attend un slot de téléchargement disponible |
| **Téléchargement** | Téléchargement depuis Telegram en progression (barre de progression visible) |
| **Importé** | Fichier téléchargé et copié dans la bibliothèque |
| **Erreur** | Échec du téléchargement ou de la copie |

**Actions disponibles** :
- **↺ Retry** — relancer le téléchargement pour les entrées en erreur ou déjà importées
- **Annuler** — interrompre un téléchargement en attente ou en cours (libère immédiatement le slot)
- **Supprimer** — retirer une entrée terminée de l'historique

**Filtres** de la page : *Actifs* (inclut En attente + Téléchargement), *Importés* et *Erreur* s'appliquent aussi aux entrées Telegram.

---

:::note[Détails techniques]
**Bibliothèque client** : [grammers](https://github.com/Lonami/grammers) — client MTProto Rust implémentant le protocole Telegram natif.

**Session** : stockée encodée en base64 dans `app_settings` (clé `telegram_monitor`, champ `session_data`). Mise à jour après chaque opération pour refléter les dernières clés de chiffrement.

**Recherche** : `client.search_messages(&chat).query(series_name).filter(InputMessagesFilterDocument)` — utilise la recherche full-text native de Telegram, côté serveur. Retourne uniquement les messages contenant des documents (pas les photos ni les messages texte).

**Déduplication** : `INSERT ... ON CONFLICT (source_id, message_id) DO UPDATE SET volume_number = EXCLUDED.volume_number WHERE volume_number IS NULL` — un fichier déjà connu n'est jamais réinséré, sauf pour corriger un `volume_number` manquant.

**Attribution `@channel`** : certains channels ajoutent automatiquement un suffixe `@username` au nom des fichiers (ex. `Berserk - 32@BD_fr.cbz`). Ce suffixe est ignoré lors de l'extraction du nom de série et du numéro de volume.

**Correspondance de séries** : comparaison `LOWER(unaccent(series_name)) = LOWER(unaccent(name))` — insensible à la casse et aux accents. "One Piece" et "one piece" sont considérés identiques.

**Répertoire de destination** : résolu dans cet ordre — (1) fichier existant de la série en DB, (2) répertoire existant dans la bibliothèque dont le nom correspond, (3) nouveau répertoire `bibliothèque/nom-de-série`.

**Nommage à l'import** : applique `rename_format`, `rename_format_hs` ou `rename_format_int` selon le type de volume détecté. Le padding de `{volume_padded}` utilise le plus grand volume connu, `series.total_volumes` si disponible, puis le volume extrait du fichier Telegram.

**Réimport** : les liens Telegram `imported` sont réconciliés avant l'affichage. Si aucun livre correspondant à la série et au volume n'existe plus en base après un délai de sécurité, le lien repasse à `available`.

**Job de sync** : type `telegram_sync`, `library_id = NULL`. Déclenchement manuel depuis la page Tâches. Un seul job actif à la fois — les doublons sont ignorés.

**Statuts de téléchargement** : `queued` → en file d'attente (semaphore plein), `downloading` → en cours, `imported` → terminé, `failed` → erreur. Le semaphore limite le nombre de téléchargements simultanés (configurable dans Settings → Telegram Monitor, défaut 2). Annuler un téléchargement `queued` ou `downloading` libère immédiatement le slot via un `AbortHandle` tokio.

**Reset au démarrage** : les téléchargements bloqués en statut `queued` ou `downloading` au redémarrage de l'API sont automatiquement réinitialisés à `available`.
:::
