---
title: Renommage
description: Renommer les livres d'une série avec un template
---

![Modal de renommage avec aperçu avant/après pour chaque livre](/screenshots/books-renaming.png)

## À quoi ça sert

Le renommage permet de standardiser les noms de fichiers de toute une série en appliquant un template cohérent. Par exemple, transformer une collection aux noms hétérogènes en une série bien nommée comme `Dragon Ball - T01 - Le Super Saiyen`.

## Comment renommer

Depuis la page d'une série, ouvrez le menu d'actions et choisissez **Renommer les livres**. Un modal s'ouvre avec :

1. **Les templates** à personnaliser (un par type de volume : régulier, hors-série, intégrale, one-shot)
2. **Un aperçu avant/après** pour chaque livre — vous voyez exactement le résultat avant d'appliquer

Une fois satisfait de l'aperçu, confirmez pour renommer les fichiers sur le disque.

## Corriger un tome ou un type de volume

Dans l'aperçu, chaque ligne expose deux champs modifiables :

- **Type** : requalifie le livre (`Régulier`, `Hors-Série`, `Intégrales`, `One-shot`) et applique le template correspondant
- **Tome** : corrige le numéro de tome utilisé par `{volume_padded}` et `{volume}` (désactivé pour les one-shots)

Modifier une de ces valeurs régénère l'aperçu côté serveur. Utile quand l'indexeur a mal détecté le numéro ou le type d'un livre : la correction est appliquée au renommage, sans passer par l'édition des métadonnées.

Le renommage **ne modifie pas le titre** stocké en base : seuls le nom de fichier, le numéro de tome et le type de volume sont mis à jour.

## Renommage automatique à l'import

Le même template est aussi utilisé lors des imports automatiques :

- **Prowlarr / qBittorrent** — quand un torrent terminé est importé dans la bibliothèque
- **Telegram Monitor** — quand un fichier Telegram est téléchargé directement dans une série

Si la série possède déjà des livres, Stripstream continue de privilégier le nommage existant comme référence pour rester cohérent avec les fichiers déjà présents. Si la série n'a pas encore de livre, le fichier importé est renommé avec le template configuré.

Exemple avec le template `{series_name} - T{volume_padded}` :

| Fichier source | Série | Résultat |
|----------------|-------|----------|
| `Frieren Tome 3 - Le voyage.cbz` | Frieren | `Frieren - T03.cbz` |

Le titre du fichier source n'est ajouté que si le template contient `{title}`. Avec `{series_name} - T{volume_padded} - {title}`, le même fichier deviendrait `Frieren - T03 - Frieren Tome 3 - Le voyage.cbz`.

## Templates par défaut

| Type | Template par défaut |
|------|-------------------|
| Régulier | `{series_name} - T{volume_padded} - {title}` |
| Hors-série | `{series_name} - HS {volume_padded}` |
| Intégrale | `{series_name} - INT {volume_padded}` |
| One-shot | `{series_name}` |

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
Les templates sont sauvegardés dans la table `app_settings` sous les clés `rename_format`, `rename_format_hs`, `rename_format_int` et `rename_format_oneshot`. Le renommage manuel inclut une déduplication automatique : si le template produit des noms identiques pour deux livres différents, un suffixe numérique est ajouté.

Le padding de `{volume_padded}` est calculé à partir du plus grand volume connu. Pour une série sans livre, l'import automatique utilise aussi `series.total_volumes` si la série est liée à des métadonnées, puis les volumes détectés dans le téléchargement.
:::
