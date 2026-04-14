---
title: Telegram
description: Notifications en temps réel via Telegram
---

## Configuration

- Bot token et chat ID à configurer
- Toggle enable/disable
- Bouton de test de connexion dans les paramètres

## Événements

16 événements configurables individuellement :

| Catégorie | Événements |
|-----------|-----------|
| **Scans** | `scan_completed`, `scan_failed`, `scan_cancelled` |
| **Miniatures** | `thumbnail_completed`, `thumbnail_failed`, `thumbnail_cancelled` |
| **Conversion** | `conversion_completed`, `conversion_failed`, `conversion_cancelled` |
| **Métadonnées** | `metadata_approved`, `metadata_batch_completed`, `metadata_refresh_completed` |
| **Lecture** | `reading_status_match_completed`, `reading_status_match_failed`, `reading_status_push_completed`, `reading_status_push_failed` |

## Images dans les notifications

- Miniatures de couverture jointes aux notifications applicables (conversion, approbation metadata)
- Upload multipart `sendPhoto` avec fallback vers `sendMessage` texte
- Fire-and-forget : les échecs de notification ne bloquent jamais l'opération principale
