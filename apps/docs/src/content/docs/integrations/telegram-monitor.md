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
4. **Intervalle de sync** — fréquence de synchronisation automatique en minutes (`0` = désactivé)

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

Une fois authentifié, ajoutez les channels à surveiller dans la section **Channels surveillés** :

- **@username** — saisissez le nom d'utilisateur du channel Telegram (avec ou sans `@`)
- **Bibliothèque cible** — associez le channel à une bibliothèque. Les fichiers trouvés seront placés dans cette bibliothèque et les séries de cette bibliothèque seront utilisées pour les recherches.

Chaque channel peut être activé/désactivé individuellement.

:::caution
Vous devez être **membre** du channel Telegram pour que la synchronisation fonctionne. Rejoignez le channel depuis votre application Telegram avant de l'ajouter ici.
:::

---

## Synchronisation

La synchronisation parcourt chaque channel configuré et y recherche, pour chaque série de la bibliothèque associée, les fichiers correspondants.

### Comment ça marche

```
Pour chaque channel activé :
    Pour chaque série de la bibliothèque associée :
        → Recherche Telegram : query = nom de la série
        → Filtre sur les fichiers (documents CBZ/CBR/PDF/EPUB/ZIP)
        → Extraction du numéro de volume depuis le nom de fichier
        → Insertion en base (ON CONFLICT → ignoré si déjà présent)
```

Les fichiers trouvés reçoivent le statut `disponible`. Ils apparaissent ensuite dans la page Téléchargements pour les séries qui existent dans votre bibliothèque.

### Déclenchement

- **Automatique** : configurer l'intervalle dans Settings → Telegram Monitor → *Intervalle de sync*
- **Manuel** : bouton *Synchroniser* dans Settings → Telegram Monitor (déclenche un job immédiat)

### Extraction du nom de série et du numéro de volume

Stripstream analyse le nom de fichier pour en extraire le nom de la série et le numéro de tome. Les formats reconnus incluent :

| Exemple | Série | Tome |
|---------|-------|------|
| `One Piece - Tome 47.cbz` | One Piece | 47 |
| `Berserk - 32@BD_fr.cbz` | Berserk | 32 |
| `Naruto - Vol. 3.cbz` | Naruto | 3 |
| `Toriko T12.cbz` | Toriko | 12 |
| `Dandadan #Ch05.cbz` | Dandadan | — |

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

:::note
Seuls les livres dont la série existe dans votre bibliothèque sont affichés ici. Les livres non associés restent en base mais n'apparaissent pas dans cet écran — utilisez la [recherche depuis la fiche série](#recherche-depuis-une-série) pour les retrouver.
:::

---

## Recherche depuis une série

Sur la fiche d'une série, le menu *Actions* → section *Téléchargement* expose le bouton **Rechercher sur Telegram**.

Cette recherche est **live** : elle interroge directement l'API Telegram en temps réel sur tous vos channels configurés, puis affiche les résultats. Elle complète la synchronisation périodique et permet de trouver des fichiers pour n'importe quelle requête, même si la série n'est pas encore dans votre bibliothèque.

Dans la fenêtre de recherche :

- **Champ de recherche** — modifiable pour affiner la requête (le nom de la série est pré-rempli)
- **Badge de la série** — relance la recherche avec le nom exact de la série
- **Résultats** — liste des fichiers trouvés avec : numéro de tome, channel source, taille, statut

Pour chaque résultat au statut *Disponible* :
- **Bouton télécharger** — lance le téléchargement immédiat
- **Bouton ignorer** — masque ce fichier

Les résultats déjà en cours de téléchargement ou déjà importés affichent leur statut sans action possible.

---

## Téléchargement

Lorsque vous cliquez sur **Télécharger** (depuis la page Téléchargements ou depuis la recherche série) :

1. Stripstream se connecte à Telegram et récupère le message contenant le fichier
2. Le fichier est téléchargé dans le répertoire de la série (détecté depuis la bibliothèque ou créé si inexistant)
3. Un job de scan est déclenché automatiquement pour intégrer le nouveau fichier
4. Le statut passe à `importing` pendant le téléchargement, puis `imported`

Le téléchargement s'effectue en arrière-plan — vous pouvez continuer à utiliser Stripstream pendant ce temps.

---

## Suivi dans la page Téléchargements

Les téléchargements Telegram apparaissent dans l'historique en haut de la page Téléchargements, mêlés aux téléchargements qBittorrent, triés par date de mise à jour.

| Statut | Description |
|--------|-------------|
| **En cours** | Téléchargement depuis Telegram en progression |
| **Importé** | Fichier téléchargé et copié dans la bibliothèque |
| **Erreur** | Échec du téléchargement ou de la copie |

**Actions disponibles** :
- **Retry** — relancer le téléchargement pour les entrées en erreur
- **Annuler / Supprimer** — interrompre ou retirer l'entrée (avec confirmation)

**Filtres** de la page : *Actifs*, *Importés* et *Erreur* s'appliquent aussi aux entrées Telegram.

---

:::note[Détails techniques]
**Bibliothèque client** : [grammers](https://github.com/Lonami/grammers) — client MTProto Rust implémentant le protocole Telegram natif.

**Session** : stockée encodée en base64 dans `app_settings` (clé `telegram_monitor`, champ `session_data`). Mise à jour après chaque opération pour refléter les dernières clés de chiffrement.

**Recherche** : `client.search_messages(&chat).query(series_name).filter(InputMessagesFilterDocument)` — utilise la recherche full-text native de Telegram, côté serveur. Retourne uniquement les messages contenant des documents (pas les photos ni les messages texte).

**Déduplication** : `INSERT ... ON CONFLICT (source_id, message_id) DO NOTHING` — un fichier déjà connu n'est jamais réinséré.

**Attribution `@channel`** : certains channels ajoutent automatiquement un suffixe `@username` au nom des fichiers (ex. `Berserk - 32@BD_fr.cbz`). Ce suffixe est ignoré lors de l'extraction du nom de série et du numéro de volume.

**Correspondance de séries** : comparaison `LOWER(unaccent(series_name)) = LOWER(unaccent(name))` — insensible à la casse et aux accents. "One Piece" et "one piece" sont considérés identiques.

**Répertoire de destination** : résolu dans cet ordre — (1) fichier existant de la série en DB, (2) répertoire existant dans la bibliothèque dont le nom correspond, (3) nouveau répertoire `bibliothèque/nom-de-série`.

**Job de sync** : type `telegram_sync`, `library_id = NULL`. Un seul job actif à la fois — les doublons sont ignorés.
:::
