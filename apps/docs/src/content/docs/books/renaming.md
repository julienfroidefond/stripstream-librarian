---
title: Renommage
description: Renommer les livres d'une série avec un template
---

![Modal de renommage avec aperçu avant/après pour chaque livre](/screenshots/books-renaming.png)

## Templates de renommage

Trois templates séparés selon le type de volume :

| Type | Template par défaut |
|------|-------------------|
| Regular | `{series_name} - T{volume_padded} - {title}` |
| Hors-série | `{series_name} - HS {volume_padded}` |
| Intégrale | `{series_name} - INT {volume_padded}` |

## Variables disponibles

| Variable | Description |
|----------|-------------|
| `{series_name}` | Nom de la série |
| `{volume}` | Numéro du volume |
| `{volume_padded}` | Numéro avec padding (auto-ajusté) |
| `{title}` | Titre du livre |
| `{authors}` | Auteurs |
| `{publish_date}` | Date de publication |
| `{isbn}` | ISBN |

## Fonctionnalités

- **Preview mode** (dry-run) avant exécution
- **Déduplication** : suffixe ajouté si le template produit des doublons
- **Nettoyage automatique** : les segments contenant des variables non définies sont supprimés proprement
- **Padding auto** : s'ajuste à la largeur du volume le plus grand de la série
- Templates sauvegardés dans `app_settings` (`rename_format`, `rename_format_hs`, `rename_format_int`)
