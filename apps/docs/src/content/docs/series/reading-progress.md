---
title: Progression de lecture
description: Suivre et mettre à jour la progression de lecture par livre et par série
---

La progression de lecture est suivie par livre, par utilisateur. Chaque livre peut être dans l'un des trois états suivants :

| État | Description |
|------|-------------|
| `unread` | Non lu (état par défaut) |
| `reading` | En cours de lecture — la page courante est mémorisée |
| `read` | Lu |

Lorsqu'une application externe (ex. KOReader, Panels) ouvre un livre via l'API, elle peut mettre à jour le statut et la page courante en temps réel. La date `last_read_at` est mise à jour automatiquement à chaque changement.

---

## Marquer des livres depuis le backoffice

### Sur la page d'une série

La liste des livres affiche le statut de chaque tome. Pour modifier le statut d'un livre individuel, utilisez le **menu contextuel** (icône ⋯) à côté du livre — les options disponibles dépendent du statut actuel (`Marquer comme lu`, `Marquer comme en cours`, `Marquer comme non lu`).

### Marquer toute la série comme lue

Sur la page d'une série, le bouton **Marquer tout comme lu** (ou l'option équivalente dans le menu d'actions) passe tous les livres de la série en `read` pour l'utilisateur courant.

---

## Statut au niveau série

Le statut d'une série est calculé automatiquement depuis les statuts de ses livres :

| Condition | Statut série |
|-----------|-------------|
| Tous les livres sont `read` | `read` |
| Aucun livre n'est `read` ou `reading` | `unread` |
| Mix (au moins un `reading` ou `read`, mais pas tous `read`) | `reading` |

Ce statut agrégé est affiché sur les cartes de séries, dans les filtres, et dans les graphiques du dashboard.

---

## Filtrer par utilisateur

Si plusieurs utilisateurs sont configurés, un sélecteur de lecteur est disponible sur la page d'une série et sur le dashboard. Chaque utilisateur a son propre historique — les modifications pour un utilisateur n'affectent pas les autres.

---

## Synchronisation avec des providers externes

La progression peut être synchronisée bidirectionnellement avec AniList (push et pull) ou importée depuis Komga (pull uniquement). Ces synchronisations sont configurées dans **Settings → onglet AniList / Komga**.

Voir [AniList](/integrations/anilist/) et [Komga](/integrations/komga/) pour le détail.
