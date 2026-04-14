---
title: AniList
description: Synchronisation de la progression de lecture avec AniList
---

## Configuration

- User ID AniList requis pour les opérations pull/push
- Configuré par bibliothèque dans les paramètres de reading status
- Planning auto-push par bibliothèque : `manual`, `hourly`, `daily`, `weekly`

## Pull — Reading Status Match

Tire la progression depuis AniList et met à jour les statuts locaux.

| Statut AniList | Statut local |
|---------------|-------------|
| `PLANNING` | `unread` |
| `CURRENT` | `reading` |
| `COMPLETED` | `read` |

- Rapport détaillé par série : matched, updated, skipped, errors
- Rate limiting : attente 10s et retry sur HTTP 429, abandon au second 429

## Push — Reading Status Push

Push différentiel : ne synchronise que les séries modifiées depuis le dernier push.

| Statut local | Statut AniList |
|-------------|---------------|
| `unread` | `PLANNING` |
| `reading` | `CURRENT` |
| `read` | `COMPLETED` |

:::caution
Ne marque jamais une série comme complétée sur AniList en se basant uniquement sur les livres possédés — il faut que tous les livres soient lus.
:::

- Résultat par série : pushed, skipped, no_books, error
- Planning auto-push vérifié chaque minute par le scheduler de l'indexer
