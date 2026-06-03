---
title: Tâches de miniatures
description: Génération et regénération des miniatures de couverture
---

Les miniatures sont générées automatiquement lors du scan (phase d'analyse). Ces tâches permettent de les regénérer manuellement si nécessaire.

---

## Générer les miniatures manquantes

Génère les miniatures uniquement pour les livres qui n'en ont pas encore. Les miniatures existantes ne sont pas touchées.

**Quand l'utiliser :** après un import manuel de fichiers qui n'ont pas été analysés, ou après une résolution d'un problème système qui avait empêché leur génération.

---

## Regénérer toutes les miniatures

:::caution
Supprime toutes les miniatures existantes d'une bibliothèque avant de les recréer. Les couvertures sont temporairement absentes pendant l'exécution. Sur une grande bibliothèque, cela peut prendre plusieurs minutes.
:::

**Quand l'utiliser :** si des miniatures sont corrompues, ou après un changement des paramètres de génération.

---

## Conversion CBR → CBZ

Convertit un fichier CBR (RAR) en CBZ (ZIP). Cette tâche est non-exclusive et peut s'exécuter en parallèle avec d'autres tâches.

Pour lancer une conversion, accédez à la page détail d'un livre CBR et utilisez l'option **Convertir en CBZ**.

Voir [Conversion CBR → CBZ](/books/conversion/) pour plus de détails.

:::note[Détails techniques]
Les deux tâches de miniatures sont **exclusives** — elles ne coexistent pas avec d'autres tâches exclusives sur la même bibliothèque. Priorité 3.

`thumbnail_rebuild` : status `generating_thumbnails` (pas de phase discovery). Ne traite que les livres sans miniature (`thumbnail_path IS NULL`).

`thumbnail_regenerate` : supprime les fichiers WebP existants pour la bibliothèque, puis recrée chaque miniature.

`cbr_to_cbz` : non-exclusif. Nécessite `book_id` sur le job. Pipeline : copie CBR → extraction `unar` → repackage ZIP `.cbz` → mise à jour `book_files` en DB. CBR original conservé en cas d'échec.

**Événement de job** : `event_type = 'error'` (niveau `error`) pour les échecs d'ouverture d'archive ou de conversion.

**API** :
```
POST /index/thumbnails/rebuild
{ "library_id": "uuid" }

POST /index/thumbnails/regenerate
{ "library_id": "uuid" }
```
:::
