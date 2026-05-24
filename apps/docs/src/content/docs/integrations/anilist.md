---
title: AniList
description: Synchronisation de la progression de lecture avec AniList
---

L'intégration AniList permet de synchroniser votre progression de lecture dans les deux sens : importer depuis AniList ce que vous avez lu, ou exporter depuis Stripstream vers votre liste AniList.

## Configuration

### 1 — Créer une application AniList

1. Connectez-vous sur [anilist.co](https://anilist.co) → votre profil → **Developer** → **Create new client**
2. Donnez un nom à l'application (ex. `Stripstream`)
3. Renseignez l'URL de callback : `http://votre-instance:7082/anilist/callback`
4. Copiez le **Client ID** affiché

### 2 — Configurer dans Stripstream

Dans **Settings → onglet Statut de lecture** :

![Page de configuration AniList dans les paramètres](/screenshots/anilist-config.png)

1. Collez le **Client ID** dans le champ dédié
2. Cliquez sur **Connecter** — une fenêtre s'ouvre sur AniList pour autoriser l'accès
3. Après autorisation, vous êtes redirigé vers Stripstream. Le token est sauvegardé automatiquement
4. Cliquez sur **Tester la connexion** pour vérifier — votre nom d'utilisateur AniList s'affiche si tout est correct

:::note
Si vous préférez entrer un token manuellement (ex. token longue durée), utilisez la section **Token manuel** (dépliable sous le bouton Connecter). Vous pouvez aussi y renseigner votre User ID AniList directement si la détection automatique échoue.
:::

### 3 — Associer un utilisateur local

Dans la section **Utilisateur local**, sélectionnez quel utilisateur Stripstream est synchronisé avec ce compte AniList. La progression de lecture de cet utilisateur sera utilisée pour les push/pull.

### 4 — Configurer par bibliothèque

Dans les paramètres de chaque bibliothèque (icône ⚙️), section **État de lecture** :
- **Provider** : sélectionnez `AniList`
- **Synchronisation automatique** : choisissez la fréquence de push automatique (`Manuel`, `Toutes les heures`, `Quotidien`, `Hebdomadaire`)

---

## Pull — Importer depuis AniList

Le pull tire la progression depuis votre liste AniList et met à jour les statuts de lecture locaux.

| Statut AniList | Statut Stripstream |
|---------------|-------------------|
| `PLANNING` | `unread` |
| `CURRENT` | `reading` |
| `COMPLETED` | `read` |

**Déclenchement** : bouton **Pull depuis AniList** dans Settings → AniList, ou job `reading_status_match`.

Le rapport détaille par série : matched, updated, skipped, errors.

---

## Push — Exporter vers AniList

Le push envoie votre progression locale vers AniList. Seules les séries **modifiées depuis le dernier push** sont envoyées (push différentiel).

| Statut Stripstream | Statut AniList |
|-------------------|---------------|
| `unread` | `PLANNING` |
| `reading` | `CURRENT` |
| `read` | `COMPLETED` |

:::caution
Ne marque jamais une série comme `COMPLETED` sur AniList en se basant uniquement sur les livres **possédés** — il faut que tous les livres soient marqués **lus**.
:::

**Déclenchement** : bouton **Push vers AniList** dans Settings → AniList, ou automatiquement selon la fréquence configurée par bibliothèque.

**Prévisualisation** : le bouton **Prévisualiser** affiche ce qui sera envoyé sans effectuer le push — utile pour vérifier avant la première synchronisation.

---

## Linking par série

Sur la page d'une série, si elle est liée à AniList via les métadonnées, vous pouvez gérer le lien AniList individuellement : voir le statut actuel sur AniList, modifier manuellement le statut ou le nombre de tomes lus.

---

## Rate limiting

- Retry automatique avec attente de 10s sur HTTP 429
- Abandon au second 429 consécutif pour éviter les bans
- Push auto vérifié chaque minute par le scheduler de l'indexer
