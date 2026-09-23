---
title: Gestion des genres
description: Taguer, renommer et filtrer les séries par genre
---

Les genres sont des tags libres attachés aux séries. Ils peuvent être issus des providers de métadonnées ou définis manuellement.

## Page Genres

![Page de gestion des genres — bibliothèque de genres et espace d'attribution](/screenshots/genres-management.png)

La page **Genres** du backoffice centralise toute la gestion :

- **Bibliothèque de genres** — chaque genre affiche une couverture représentative et son nombre de séries. Cliquez dessus pour consulter les séries concernées.
- **Renommage et suppression** — ces actions se trouvent dans la modale du genre. Le renommage s'applique à toutes les séries ; la suppression retire uniquement ce genre des séries.
- **Espace d'attribution** — filtrez les séries par bibliothèque, genre inclus ou exclu, ou affichez uniquement les séries sans genre. Le filtre **Tous** rétablit toutes les séries du périmètre sélectionné. La recherche porte sur le nom de série, sans distinction de casse ni d'accents (par ex. `asterix` retrouve `Astérix`), et s'applique à l'ensemble du périmètre filtré — pas seulement aux séries affichées.
- **Sélection et affichage** — choisissez plusieurs séries dans la vue cartes ou tableau, puis affectez-leur un genre existant en une opération.

## Taguer des séries

### Assignation en masse

Depuis la page Genres, sélectionnez les séries à traiter — notamment via le filtre **Séries sans genre** — puis assignez-leur un genre en une opération.

### Assignation individuelle

Sur la page détail d'une série, les genres sont modifiables directement dans les métadonnées.

### Propositions par IAG

Dans **Paramètres → IAG**, configurez l'endpoint OpenAI-compatible, la clé API, le modèle et le prompt (OpenRouter est préconfiguré). Sélectionnez les séries à analyser puis lancez les propositions. L'IAG choisit uniquement parmi les genres déjà existants ; aucun nouveau genre ne peut être créé par cette fonctionnalité. Les suggestions sont regroupées dans une zone de révision par série et doivent être affectées individuellement : aucune modification n'est appliquée automatiquement.

Le prompt par défaut autorise les connaissances générales fiables pour les titres connus, même lorsque leurs métadonnées sont limitées. Les séries qui ne peuvent pas être identifiées avec confiance sont omises. Lors de la mise à jour, seul l’ancien prompt par défaut est remplacé ; les prompts personnalisés et les autres paramètres IAG sont conservés.

## Filtrer les séries par genre

Sur la page **Séries**, un filtre par genre est disponible. Les séries peuvent être filtrées par un ou plusieurs genres.

## Genres et synchronisation metadata

Les genres issus des providers sont écrits lors de la synchronisation des métadonnées. Pour éviter qu'un rafraîchissement n'écrase vos genres personnalisés, verrouillez le champ **genres** sur la série.

:::note[Détails techniques]
**API Genres** :

| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `GET` | `/genres` | Liste tous les genres avec compteur de séries |
| `PATCH` | `/genres/{name}` | Renomme un genre sur toutes les séries |
| `DELETE` | `/genres/{name}` | Supprime un genre de toutes les séries |
| `POST` | `/genres/assign` | Assigne un genre à une liste de séries (bulk) |
| `GET` | `/genres/untagged-series` | Séries sans aucun genre (max 500) |
| `POST` | `/genres/ai-suggest` | Propose des tags pour 1 à 25 séries sans modifier les données |

`GET /genres` et `GET /genres/untagged-series` acceptent le paramètre `library_id` (UUID) pour filtrer à une bibliothèque donnée. `GET /genres/untagged-series`, `GET /series` et `GET /libraries/{library_id}/series` acceptent aussi `q` pour filtrer par nom de série (recherche partielle, insensible à la casse et aux accents).
:::
