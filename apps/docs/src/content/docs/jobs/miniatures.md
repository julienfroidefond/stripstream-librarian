---
title: Jobs de miniatures
description: Génération et regénération des miniatures — thumbnail_rebuild, thumbnail_regenerate, cbr_to_cbz
---

Les jobs de miniatures sont exécutés par le service **Indexer**. Les deux jobs de miniatures sont **exclusifs** (ne coexistent pas avec d'autres jobs exclusifs) ; `cbr_to_cbz` est non-exclusif.

---

## `thumbnail_rebuild` — Générer les miniatures manquantes

Génère les miniatures uniquement pour les livres qui n'en ont pas encore.

### Règles métier

- Ne traite que les livres sans miniature existante
- Ouvre chaque archive, extrait la première image, génère un fichier WebP redimensionné
- Les miniatures existantes ne sont **pas touchées**
- Status du job : passe directement à `generating_thumbnails` (pas de phase discovery)
- Priorité 3

### Cas d'usage

Après un import manuel de fichiers qui n'ont pas été analysés, ou après une résolution de problème avec `pdfinfo`/`pdftoppm` qui avait empêché la génération.

### API

```
POST /index/thumbnails/rebuild
{ "library_id": "uuid" }   // optionnel
```

---

## `thumbnail_regenerate` — Regénérer toutes les miniatures ⚠️

Supprime toutes les miniatures existantes d'une bibliothèque et les recrée depuis zéro.

### Règles métier

- Supprime les fichiers WebP existants pour tous les livres de la bibliothèque
- Recrée chaque miniature en ouvrant l'archive et extrayant la première image
- Status du job : `generating_thumbnails`
- Priorité 3

### Cas d'usage

Après un changement de résolution ou de qualité des miniatures, ou si des miniatures sont corrompues.

:::caution
Les miniatures sont temporairement absentes pendant l'exécution du job. Sur une grande bibliothèque, cela peut prendre plusieurs minutes.
:::

### API

```
POST /index/thumbnails/regenerate
{ "library_id": "uuid" }   // optionnel
```

---

## `cbr_to_cbz` — Conversion CBR → CBZ

Convertit un livre CBR en CBZ. Job **non-exclusif** : peut s'exécuter en parallèle avec d'autres jobs.

### Règles métier

- Déclenché depuis la page détail d'un livre CBR
- Nécessite un `book_id` sur le job (ne fonctionne pas sans)
- Pipeline de conversion :
  1. Copie le fichier CBR dans un dossier temporaire
  2. Extrait les images via `unar`
  3. Repackage en archive ZIP avec extension `.cbz`
  4. Remplace le fichier d'origine sur le disque
  5. Met à jour `book_files` en DB avec le nouveau chemin et format
- En cas d'échec, le fichier CBR original est conservé

### Prérequis système

Les outils `unrar` (listing) et `unar` (extraction) doivent être installés. Ce sont deux outils distincts issus de paquets différents.

---

## Événements de job

| `event_type` | Niveau | Signification |
|-------------|--------|---------------|
| `error` | `error` | Échec d'ouverture d'archive ou de conversion |
