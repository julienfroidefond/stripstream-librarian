---
title: Listes de lecture
description: Organiser ses séries en collections ordonnées personnalisées
---

Les listes de lecture permettent de regrouper des séries en collections nommées et ordonnées. Elles servent à structurer une bibliothèque : créer un ordre de lecture pour une saga, regrouper des séries par thème ou par priorité, constituer une pile "à lire".

---

## Créer une liste

Depuis la page **Listes** (menu de navigation), cliquez sur **Créer une liste**. Renseignez :

- **Nom** (obligatoire)
- **Description** (optionnelle)

La liste apparaît immédiatement dans la grille des listes.

---

## Ajouter des séries

### Depuis une liste

Sur la page détail d'une liste, cliquez sur **Ajouter une série**. Une modale de recherche s'ouvre — tapez le nom d'une série pour la trouver et l'ajouter en un clic. Les séries déjà présentes dans la liste apparaissent désactivées.

### Depuis la page Séries

Sur la page **Séries**, le menu contextuel de chaque carte propose **Ajouter à une liste**. Une modale liste vos collections existantes ; cliquez sur celle de votre choix. La confirmation visuelle (coche verte) confirme l'ajout.

### Depuis la page détail d'une série

Sur la page d'une série, la section **Listes de lecture** indique les listes dont elle fait déjà partie et propose un bouton pour l'ajouter à une nouvelle liste.

---

## Réordonner les séries

Dans la page détail d'une liste, chaque série dispose de boutons **↑ / ↓** (visibles au survol) pour la déplacer dans la liste. L'ordre est persisté immédiatement côté serveur.

---

## Modifier ou supprimer une liste

**Modifier le nom/description** : sur la page détail, cliquez sur l'icône crayon à côté du titre.

**Supprimer une liste** : sur la page d'accueil des listes, survolez une carte et cliquez sur l'icône corbeille qui apparaît en haut à droite. Une confirmation est demandée. La suppression est définitive mais n'affecte pas les séries elles-mêmes.

---

## Grouper les séries par liste

Sur la page **Séries**, le toggle **Grouper par liste** (à côté des filtres) réorganise la vue en regroupant les séries sous leur(s) liste(s) de lecture. Les séries sans liste apparaissent dans une section séparée.

---

## Impact sur les recommandations

Les listes de lecture enrichissent le moteur de séries liées : deux séries dans une même liste obtiennent un **bonus de score ×5**, supérieur au bonus auteur (×3) ou genre (×2). Cela permet de retrouver facilement des séries d'une même saga ou d'un même univers dans les recommandations.

Voir [Séries liées & Recommandations](/series/related/) pour le détail du scoring.

---

## API

| Méthode | Endpoint | Description |
|---------|----------|-------------|
| `GET` | `/reading-lists` | Lister toutes les listes |
| `POST` | `/reading-lists` | Créer une liste |
| `GET` | `/reading-lists/{id}` | Détail d'une liste avec ses séries |
| `PATCH` | `/reading-lists/{id}` | Modifier le nom/description |
| `DELETE` | `/reading-lists/{id}` | Supprimer une liste |
| `POST` | `/reading-lists/{id}/series` | Ajouter une série |
| `DELETE` | `/reading-lists/{id}/series/{series_id}` | Retirer une série |
| `PUT` | `/reading-lists/{id}/series/reorder` | Réordonner (liste complète d'IDs) |
