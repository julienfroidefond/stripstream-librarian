---
title: Telegram Monitor
description: Surveiller des channels Telegram pour télécharger automatiquement des livres (CBZ, CBR, PDF, EPUB)
---

Telegram Monitor surveille des channels Telegram à la recherche de livres (CBZ, CBR, PDF, EPUB) partagés en tant que documents. Quand un fichier correspondant à une série de votre bibliothèque est détecté, vous pouvez le télécharger en un clic directement depuis Stripstream — sans client torrent ni indexeur externe.

Pour le flux de téléchargement et le suivi des statuts, voir [Téléchargements — Telegram](/downloads/telegram).

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

### Sync incrémentale (automatique)

La sync incrémentale parcourt les **nouveaux messages** depuis la dernière exécution pour chaque channel. Elle traite tous les fichiers trouvés sans filtre sur les séries.

Elle tourne en arrière-plan à la fréquence configurée (30 min par défaut) dès que vous êtes authentifié — aucune configuration nécessaire.

### Sync complète (manuelle)

La sync complète effectue une **recherche active par série** sur chaque channel : elle interroge l'API Telegram avec le nom de chaque série éligible comme requête de recherche.

:::note[Quelles séries sont recherchées ?]
La sync complète ne recherche **pas** toutes les séries de votre bibliothèque. Pour être incluse, une série doit remplir deux conditions :

1. **Lien metadata approuvé** — un lien vers un provider externe (AniList, BDTheque…) avec statut *approuvé*
2. **Volumes manquants** — au moins un tome référencé dans les metadata mais absent de votre bibliothèque

Une série qui possède une **intégrale** est considérée complète et n'est pas recherchée automatiquement.

C'est intentionnel : seules les séries que vous cherchez activement à compléter sont scrutées, ce qui limite le nombre de requêtes Telegram.

Si une série n'a pas de metadata liée, utilisez la [recherche depuis la fiche série](/downloads/telegram#recherche-depuis-une-série) pour la trouver manuellement.
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

:::note[Détails techniques]
**Bibliothèque client** : [grammers](https://github.com/Lonami/grammers) — client MTProto Rust implémentant le protocole Telegram natif.

**Session** : stockée encodée en base64 dans `app_settings` (clé `telegram_monitor`, champ `session_data`). Mise à jour après chaque opération pour refléter les dernières clés de chiffrement.

**Recherche** : `client.search_messages(&chat).query(series_name).filter(InputMessagesFilterDocument)` — utilise la recherche full-text native de Telegram, côté serveur. Retourne uniquement les messages contenant des documents (pas les photos ni les messages texte).

**Déduplication** : `INSERT ... ON CONFLICT (source_id, message_id) DO UPDATE SET volume_number = EXCLUDED.volume_number WHERE volume_number IS NULL` — un fichier déjà connu n'est jamais réinséré, sauf pour corriger un `volume_number` manquant.

**Attribution `@channel`** : certains channels ajoutent automatiquement un suffixe `@username` au nom des fichiers (ex. `Berserk - 32@BD_fr.cbz`). Ce suffixe est ignoré lors de l'extraction du nom de série et du numéro de volume.

**Correspondance de séries** : comparaison `LOWER(unaccent(series_name)) = LOWER(unaccent(name))` — insensible à la casse et aux accents.

**Job de sync** : type `telegram_sync`, `library_id = NULL`. Un seul job actif à la fois — les doublons sont ignorés.
:::
