---
title: Renommage
description: Renommer les livres d'une série avec un template
---

![Modal de renommage avec aperçu avant/après pour chaque livre](/screenshots/books-renaming.png)

## À quoi ça sert

Le renommage permet de standardiser les noms de fichiers de toute une série en appliquant un template cohérent. Par exemple, transformer une collection aux noms hétérogènes en une série bien nommée comme `Dragon Ball - T01 - Le Super Saiyen`.

## Comment renommer

Depuis la page d'une série, ouvrez le menu d'actions et choisissez **Renommer les livres**. Un modal s'ouvre avec :

1. **Le template** à personnaliser (un par type de volume : régulier, hors-série, intégrale)
2. **Un aperçu avant/après** pour chaque livre — vous voyez exactement le résultat avant d'appliquer

Une fois satisfait de l'aperçu, confirmez pour renommer les fichiers sur le disque.

## Templates par défaut

| Type | Template par défaut |
|------|-------------------|
| Régulier | `{series_name} - T{volume_padded} - {title}` |
| Hors-série | `{series_name} - HS {volume_padded}` |
| Intégrale | `{series_name} - INT {volume_padded}` |

## Variables disponibles

| Variable | Valeur insérée |
|----------|---------------|
| `{series_name}` | Nom de la série |
| `{volume}` | Numéro du volume |
| `{volume_padded}` | Numéro avec zéros (auto-ajusté selon le plus grand tome) |
| `{title}` | Titre du livre |
| `{authors}` | Auteurs |
| `{publish_date}` | Date de publication |
| `{isbn}` | ISBN |

Les segments contenant des variables non renseignées (ex. `{title}` quand il n'y a pas de titre) sont supprimés proprement, sans laisser de tirets orphelins.

:::tip
Utilisez `{volume_padded}` plutôt que `{volume}` pour garantir un tri alphabétique correct (ex. `T01`, `T02`… plutôt que `T1`, `T2`…).
:::

:::note[Détails techniques]
Les templates sont sauvegardés dans la table `app_settings` sous les clés `rename_format`, `rename_format_hs` et `rename_format_int`. Le renommage inclut une déduplication automatique : si le template produit des noms identiques pour deux livres différents, un suffixe numérique est ajouté. Le padding est auto-calculé à partir du volume le plus grand de la série.
:::
