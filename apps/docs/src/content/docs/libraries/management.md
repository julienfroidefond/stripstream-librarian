---
title: Gestion des bibliothèques
description: Créer et configurer vos bibliothèques
---

## Créer une bibliothèque

Dans la page **Libraries**, renseignez un nom et choisissez un dossier racine via le sélecteur de dossiers. Le dossier doit être accessible par l'indexer (monté dans le conteneur Docker si applicable).

Une fois créée, la bibliothèque apparaît dans la grille avec ses compteurs de livres et de séries.

## Supprimer une bibliothèque

L'icône corbeille sur la carte de la bibliothèque supprime la bibliothèque et **toutes ses données en cascade** : livres, séries, jobs, liens metadata. Cette action est irréversible.

## Paramètres d'une bibliothèque

Cliquez sur l'icône engrenage ⚙️ d'une bibliothèque pour ouvrir ses paramètres. Ils sont organisés en cinq sections.

![Modal des paramètres d'une bibliothèque](/screenshots/library-settings.png)

---

### Indexation

**Scan planifié**

Active un scan automatique périodique. Le fréquence se choisit dans la liste déroulante à droite :

| Valeur | Description |
|--------|-------------|
| Manuel | Aucun scan automatique |
| Toutes les heures | Scan toutes les heures |
| Quotidien | Scan une fois par jour |
| Hebdomadaire | Scan une fois par semaine |

La carte de la bibliothèque affiche en temps réel le prochain scan prévu (ex. `Scan dans 4h`).

**Surveillance en temps réel** (watcher)

Détecte instantanément les ajouts ou suppressions de fichiers dans le dossier racine, sans attendre le prochain scan planifié. Utile si vous ajoutez régulièrement des fichiers et souhaitez qu'ils apparaissent immédiatement.

:::note
Le watcher et le scan planifié sont indépendants — vous pouvez activer l'un, l'autre, ou les deux.
:::

---

### Métadonnées

**Fournisseur principal**

Source utilisée pour récupérer les métadonnées (titre, description, couverture…) lors du matching automatique. Options :

| Valeur | Description |
|--------|-------------|
| Par défaut | Utilise le provider configuré globalement dans les Settings |
| Aucun | Désactive la recherche automatique de métadonnées |
| Google Books | Livres, romans, BD en français |
| ComicVine | Comics anglophones |
| Open Library | Catalogue mondial (livres anciens, public domain) |
| AniList | Manga, manhwa, manhua |
| Bédéthèque | BD franco-belge |
| SensCritique | BD et manga en français |

**Fournisseur de secours**

Utilisé si le fournisseur principal ne retourne aucun résultat pour une série. Permet par exemple de chercher sur AniList en priorité, puis sur SensCritique si rien n'est trouvé.

**Rafraîchissement auto des métadonnées**

Re-télécharge périodiquement les métadonnées des séries déjà matchées (nouvelles sorties, mises à jour). Même fréquences que le scan planifié. Désactivé (`Manuel`) par défaut.

---

### Tags

Permet de catégoriser la bibliothèque pour le filtrage dans l'interface. Tags prédéfinis :

`manga` · `manhwa` · `manhua` · `bd-jeunesse` · `bd-adulte` · `bd-franco-belge` · `comics` · `webtoon` · `light-novel`

Cliquez sur un tag pour l'activer ou le désactiver. Les tags actifs apparaissent sur la carte de la bibliothèque.

---

### État de lecture

**Provider d'état de lecture**

Synchronise les états de lecture (lu / en cours / planifié) avec un service externe.

| Valeur | Description |
|--------|-------------|
| Aucun | Pas de synchronisation |
| AniList | Pousse la progression vers votre compte AniList |

**Synchronisation automatique**

Fréquence à laquelle la progression de lecture est poussée automatiquement vers le provider. Si laissé à `Manuel`, la synchronisation se déclenche uniquement via le bouton dédié dans la page de la série.

---

### Détection de téléchargements

**Détection automatique**

Lance périodiquement la détection de volumes manquants via Prowlarr (si les téléchargements sont activés dans les Settings). La détection cherche dans Prowlarr les volumes absents de votre bibliothèque et les propose au téléchargement.

| Valeur | Description |
|--------|-------------|
| Manuel | Détection uniquement à la demande |
| Toutes les heures | Vérification horaire |
| Quotidien | Vérification quotidienne |
| Hebdomadaire | Vérification hebdomadaire |

:::note
Cette section n'a d'effet que si les téléchargements sont activés dans **Settings → Téléchargements**.
:::
