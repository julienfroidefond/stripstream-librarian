---
title: Progression de lecture
description: Suivre et mettre à jour la progression de lecture par livre et par série
---

La progression de lecture est suivie par livre, par utilisateur. Chaque livre peut avoir l'un des trois états suivants :

| État | Description |
|------|-------------|
| **Non lu** | État par défaut |
| **En cours** | Vous avez commencé ce livre — la page courante est mémorisée |
| **Lu** | Vous avez terminé ce livre |

---

## Marquer des livres depuis le backoffice

### Sur la page d'une série

La liste des livres affiche le statut de chaque tome. Pour modifier le statut d'un livre, utilisez le **menu contextuel** (icône ⋯) à côté du livre — les options disponibles dépendent du statut actuel.

### Marquer toute la série comme lue

Sur la page d'une série, le bouton **Marquer tout comme lu** passe tous les livres de la série en "lu" en une seule action.

---

## Statut global d'une série

Le statut d'une série est calculé automatiquement depuis les statuts de ses livres :

- **Lu** — tous les livres sont lus
- **Non lu** — aucun livre n'est lu ou en cours
- **En cours** — au moins un livre est lu ou en cours, mais pas tous

Ce statut est affiché sur les cartes de séries, dans les filtres et dans les graphiques du dashboard.

---

## Multi-utilisateurs

Si plusieurs utilisateurs sont configurés, un sélecteur de lecteur est disponible sur la page d'une série et sur le dashboard. Chaque utilisateur a sa propre progression — les modifications pour un utilisateur n'affectent pas les autres.

---

## Compte admin et progression de lecture

Le compte administrateur **ne peut pas enregistrer de progression de lecture** directement. C'est un choix de conception : l'admin n'a pas d'identité de lecteur propre.

Pour suivre votre lecture depuis le backoffice, sélectionnez un utilisateur dans le **sélecteur de lecteur** (en haut à droite de l'interface). Les boutons *Marquer comme lu* n'apparaissent que lorsqu'un utilisateur est sélectionné.

### Depuis l'API

Les tokens de scope **admin** ne peuvent pas non plus enregistrer la progression. Pour qu'une application externe (KOReader, Panels…) puisse suivre la lecture :

1. Créez un utilisateur lecteur dans Settings → Tokens
2. Créez un token **read** associé à cet utilisateur
3. Utilisez ce token dans votre application — la progression sera enregistrée pour cet utilisateur

---

## Synchronisation avec des applications externes

Votre progression peut être synchronisée avec d'autres services :

- **AniList** : synchronisation bidirectionnelle (envoi et réception)
- **Komga** : import uniquement (Komga → Stripstream)

Ces synchronisations se configurent dans **Settings → onglet AniList / Komga**.

Voir [AniList](/integrations/anilist/) et [Komga](/integrations/komga/) pour le détail.

:::note[Détails techniques]
Les états de lecture sont stockés par triplet `(book_id, user_id, status)` avec `last_read_at` mis à jour à chaque changement. La page courante est mémorisée pour l'état `reading`.

Quand une application externe (ex. KOReader, Panels) ouvre un livre via l'API, elle peut mettre à jour le statut et la page courante en temps réel.
:::
