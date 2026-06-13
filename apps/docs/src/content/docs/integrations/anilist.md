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
Si vous préférez entrer un token manuellement (ex. token longue durée), utilisez la section **Token manuel** (dépliable sous le bouton Connecter). Vous pouvez aussi y renseigner votre User ID AniList si la détection automatique échoue.
:::

### 3 — Associer un utilisateur local

Dans la section **Utilisateur local**, sélectionnez quel utilisateur Stripstream est synchronisé avec ce compte AniList. La progression de lecture de cet utilisateur sera utilisée pour les push/pull.

### 4 — Configurer par bibliothèque

Dans les paramètres de chaque bibliothèque (icône ⚙️), section **État de lecture** :
- **Provider** : sélectionnez `AniList`
- **Synchronisation automatique** : choisissez la fréquence de push automatique

---

## Importer depuis AniList (Pull)

Le pull tire votre progression depuis AniList et met à jour les statuts de lecture locaux. Les séries que vous avez "en cours" ou "terminées" sur AniList sont mises à jour dans Stripstream.

**Déclenchement** : bouton **Pull depuis AniList** dans Settings → AniList.

Le rapport détaille par série : matched, updated, skipped, errors.

---

## Exporter vers AniList (Push)

Le push envoie votre progression locale vers AniList. Seules les séries **modifiées depuis le dernier push** sont envoyées — pas besoin de tout renvoyer à chaque fois.

:::caution
Stripstream ne marque jamais une série comme terminée sur AniList en se basant uniquement sur les livres **possédés** — il faut que tous les livres soient effectivement marqués **lus** dans Stripstream.
:::

**Prévisualisation** : le bouton **Prévisualiser** affiche ce qui sera envoyé sans effectuer le push — utile pour vérifier avant la première synchronisation.

**Déclenchement** : bouton **Push vers AniList** dans Settings → AniList, ou automatiquement selon la fréquence configurée par bibliothèque.

---

## Correspondance des statuts

| Statut sur AniList | Statut dans Stripstream |
|-------------------|------------------------|
| Planifié (PLANNING) | Non lu |
| En cours (CURRENT) | En cours |
| Terminé (COMPLETED) | Lu |

---

## Gestion par série

Sur la page d'une série liée à AniList, vous pouvez consulter et modifier directement son statut AniList sans passer par une synchronisation complète.

## Notation et scores

Lorsque vous notez une série dans Stripstream (voir [Notation des séries](/series/ratings/)), la note est **automatiquement poussée vers AniList** si la série est liée. Le push utilise `SaveMediaListEntry(score)` avec une conversion vers l'échelle POINT_100.

Inversement, lors d'un **Pull**, votre score AniList est importé dans Stripstream comme note initiale (sans écraser une note locale existante).

---

:::note[Détails techniques]
Rate limiting : attente fixe de 700ms entre chaque requête AniList (~85 req/min). Retry 10s sur HTTP 429, abandon au second 429 consécutif.

Le push est différentiel : une série est envoyée si `synced_at IS NULL` ou si la progression a changé depuis `synced_at`.

Seuls les livres avec `volume_type IN ('regular', 'integral')` sont comptés pour le calcul du statut et du `progress` envoyé à AniList. Les hors-série et one-shots sont ignorés.

Push auto vérifié chaque minute par le scheduler de l'indexer.
:::
