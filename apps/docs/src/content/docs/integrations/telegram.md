---
title: Telegram
description: Notifications en temps réel via Telegram
---

L'intégration Telegram vous permet de recevoir des notifications sur votre téléphone ou ordinateur quand des événements importants se produisent dans Stripstream : nouveau téléchargement importé, scan terminé, métadonnées approuvées, etc.

## Configuration

Dans **Paramètres → onglet Notifications** :

1. **Bot token** — créez un bot Telegram via [@BotFather](https://t.me/botfather) et copiez le token
2. **Chat ID** — l'identifiant du chat où envoyer les notifications (utilisateur ou groupe)
3. **Activer** — le toggle enable/disable
4. **Tester** — un bouton de test envoie un message de vérification

---

## Événements configurables

Vous pouvez activer ou désactiver chaque type de notification individuellement :

| Catégorie | Événements disponibles |
|-----------|----------------------|
| **Scans** | Scan terminé, scan en erreur, scan annulé |
| **Miniatures** | Génération terminée, en erreur |
| **Conversion CBR→CBZ** | Conversion terminée, en erreur |
| **Métadonnées** | Métadonnées approuvées, auto-match terminé, auto-match en erreur, refresh terminé, refresh en erreur |
| **Lecture** | Liaison AniList terminée, liaison en erreur, push AniList terminé, push en erreur |
| **Téléchargements** | Détection de téléchargements terminée, détection en erreur |

## Contenu des notifications

Les notifications de succès les plus importantes incluent aussi un résumé utile, pas seulement un compteur :

- **Scan avec nouveaux fichiers** : une notification par série, avec l'affiche quand elle est disponible, puis la liste des nouveaux livres ajoutés dans cette série
- **Scan terminé sans nouveaux livres** : bibliothèque, type de scan, durée et compteurs
- **Pull AniList terminé** : nombre de séries liées, puis la liste des séries effectivement liées pendant ce job
- **Push AniList terminé** : nombre de séries poussées, puis la liste des séries effectivement envoyées à AniList
- **Synchronisation Telegram Monitor** : uniquement les nouveaux fichiers rapprochés d'une série de votre bibliothèque ; aucune notification n'est envoyée si aucun fichier ne correspond

Les listes sont volontairement tronquées si le job a produit beaucoup d'éléments.

---

## Images dans les notifications

Pour les événements pertinents (scan avec nouveaux livres, conversion, approbation de métadonnées), la miniature de couverture est jointe à la notification Telegram.

:::tip
Si une image ne s'affiche pas dans une notification, c'est que la miniature n'était pas encore générée au moment de l'envoi. La notification texte est toujours envoyée.
:::

:::note[Détails techniques]
Les notifications avec image utilisent l'API Telegram `sendPhoto` (upload multipart) avec fallback automatique vers `sendMessage` (texte seul) en cas d'échec.

Les notifications sont envoyées en mode fire-and-forget : un échec d'envoi ne bloque jamais l'opération principale qui l'a déclenchée.

Codes d'événements internes : `scan_completed`, `scan_failed`, `scan_cancelled`, `thumbnail_completed`, `thumbnail_failed`, `conversion_completed`, `conversion_failed`, `metadata_approved`, `metadata_batch_completed`, `metadata_batch_failed`, `metadata_refresh_completed`, `metadata_refresh_failed`, `reading_status_match_completed`, `reading_status_match_failed`, `reading_status_push_completed`, `reading_status_push_failed`, `download_detection_completed`, `download_detection_failed`.
:::
