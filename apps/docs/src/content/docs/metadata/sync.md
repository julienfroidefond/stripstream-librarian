---
title: Synchronisation des métadonnées
description: Enrichir les séries avec des métadonnées externes — workflow utilisateur et règles de mise à jour
---

La synchronisation des métadonnées permet d'enrichir chaque série avec des informations issues de providers externes : description, couverture, auteurs, statut de publication, liste des tomes, etc.

## Workflow utilisateur

### 1 — Rechercher des métadonnées pour une série

Sur la page d'une série, cliquez sur le bouton **Rechercher des métadonnées** (icône loupe). Une fenêtre s'ouvre avec les résultats du provider configuré pour votre bibliothèque.

![Fenêtre de recherche de métadonnées avec résultats et scores de confiance](/screenshots/metadata-search.png)

Chaque résultat affiche le titre, les auteurs, la couverture, le nombre de tomes et un score de confiance (0 à 1). Les boutons de provider en haut permettent de relancer la recherche sur un autre provider sans fermer la fenêtre.

### 2 — Approuver ou rejeter

- **Approuver** : valide le lien et synchronise immédiatement tous les champs (description, couverture, auteurs, statut, genres, tomes)
- **Rejeter** : écarte ce résultat sans synchroniser

Un seul lien peut être approuvé à la fois par série.

### 3 — Verrouiller des champs

Après synchronisation, vous pouvez modifier manuellement n'importe quel champ. Pour empêcher le prochain refresh de l'écraser, activez le **verrou** sur ce champ via l'icône cadenas à côté du champ éditable.

![Modal d'édition d'une série — les cadenas oranges indiquent les champs verrouillés](/screenshots/series-edit-locked-fields.png)

### 4 — Rafraîchir

Le bouton **Refresh** re-télécharge les données du provider et met à jour les champs non verrouillés. Utile quand un nouveau tome est sorti et que `total_volumes` doit être actualisé.

---

## Matching en masse

Plutôt que de matcher série par série, utilisez le job **Batch metadata** depuis la page Jobs :

- Traite toutes les séries sans lien approuvé
- Valide automatiquement les matchs avec un score de confiance de 1.0
- Les autres résultats sont listés dans le rapport du job pour traitement manuel

Voir [Batch & Refresh](/metadata/batch-refresh/) pour le détail des statuts de résultat.

---

## Champs synchronisés

### Série

| Champ | Règle de mise à jour |
|-------|---------------------|
| `description` | Remplace si non-vide |
| `authors` | Remplace si le tableau est non-vide |
| `publishers` | Remplace si le tableau est non-vide |
| `start_year` | Remplace si absent en base |
| `total_volumes` | Remplace si absent en base |
| `status` | Remplace si absent en base (normalisé via les mappings de statut) |
| `genres` | Remplace si le tableau est non-vide |
| `cover_url` | Remplace si non-vide |

### Livres

Pour chaque tome de la série, les champs suivants sont mis à jour :

| Champ | Règle |
|-------|-------|
| `summary` | Remplace si non-vide |
| `isbn` | Remplace si non-vide |
| `publish_date` | Remplace si non-vide |
| `language` | Remplace si non-vide |
| `authors` | Remplace si tableau non-vide |

:::important
Tous les champs respectent le **verrouillage** (`locked_fields`). Un champ verrouillé n'est jamais modifié par la synchronisation, quelle que soit la source.
:::

### Matching des livres

Les livres externes sont appariés aux livres locaux en deux étapes :

1. **Par numéro de volume** — correspondance exacte (le volume 0 = HS chez les providers est ignoré)
2. **Par titre** — containment case-insensitive si le numéro de volume n'a pas suffi

Seuls les livres `regular` et `integral` participent au matching.

---

## Scoring de confiance

Le score (0.0 → 1.0) combine la similarité de nom avec un boost selon le nombre de tomes :

| Condition | Boost |
|-----------|-------|
| Nombre de tomes identique (local == provider) | +0.30 |
| Proche (±2 tomes) | +0.15 |

Seul un score de **1.0** déclenche la validation automatique dans le job batch. En dessous, le match est proposé pour validation manuelle.

---

## Verrouillage de champs

Champs verrouillables sur une **série** : `description`, `authors`, `publishers`, `start_year`, `total_volumes`, `status`, `genres`

Champs verrouillables sur un **livre** : `summary`, `isbn`, `publish_date`, `language`, `authors`

Les rapports de synchronisation distinguent les champs mis à jour de ceux ignorés (verrouillés).

---

## Mappings de statut

Le statut retourné par les providers (`ongoing`, `ended`, `completed`…) n'est pas toujours homogène. Les **status mappings** permettent de normaliser les valeurs des providers vers vos propres labels.

Accès : **Settings → onglet Général → Status Mappings**.
