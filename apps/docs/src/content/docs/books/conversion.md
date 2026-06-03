---
title: Conversion CBR → CBZ
description: Convertir les archives RAR en ZIP
---

## Pourquoi convertir ?

Les fichiers CBR sont des archives au format RAR, moins universel que le ZIP. Les convertir en CBZ (format ZIP) améliore la compatibilité avec les lecteurs externes et facilite l'indexation.

## Comment convertir un livre

Depuis la page détail d'un livre CBR, utilisez l'option **Convertir en CBZ**. La conversion s'exécute en arrière-plan — vous pouvez suivre sa progression dans la page **Tâches**.

Ce que vous devez savoir :

- Le fichier CBR original est **conservé** jusqu'à la fin de la conversion
- Une notification Telegram peut être envoyée à la fin (si configuré)
- En cas d'échec, le fichier CBR original reste intact

:::note[Détails techniques]
La conversion nécessite les outils `unrar` (listing) et `unar` (extraction) installés dans l'environnement. Ce sont deux outils distincts issus de paquets différents — les deux sont requis.

Pipeline : copie dans un dossier temporaire → extraction des images via `unar` → repackage en ZIP avec extension `.cbz` → remplacement du fichier sur le disque → mise à jour du chemin et du format en base de données.

La conversion est un job de type `cbr_to_cbz`, non-exclusif, exécuté par l'Indexer.
:::
