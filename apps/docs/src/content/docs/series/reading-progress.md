---
title: Progression de lecture
description: Suivi de la progression de lecture
---

## Suivi par livre

| État | Description |
|------|-------------|
| `unread` | Non lu (défaut) |
| `reading` | En cours de lecture |
| `read` | Lu |

- Page courante trackée quand le statut est `reading`
- Timestamp `last_read_at` mis à jour automatiquement

## Statut au niveau série

Calculé depuis les statuts des livres :

| Condition | Statut série |
|-----------|-------------|
| Tous lus | `read` |
| Aucun lu | `unread` |
| Mix | `reading` |

## Opérations en masse

- Marquer une série entière comme lue (met à jour tous les livres)
