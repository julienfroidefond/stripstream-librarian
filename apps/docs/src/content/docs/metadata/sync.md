---
title: Synchronisation des métadonnées
description: Enrichir les séries avec des métadonnées externes — workflow utilisateur
---

La synchronisation des métadonnées permet d'enrichir chaque série avec des informations issues de providers externes : description, couverture, auteurs, statut de publication, liste des tomes, etc.

## Workflow utilisateur

### 1 — Rechercher des métadonnées pour une série

Sur la page d'une série, cliquez sur le bouton **Rechercher des métadonnées** (icône loupe). Une fenêtre s'ouvre avec les résultats du provider configuré pour votre bibliothèque.

![Fenêtre de recherche de métadonnées avec résultats et scores de confiance](/screenshots/metadata-search.png)

Chaque résultat affiche le titre, les auteurs, la couverture, le nombre de tomes et un score de confiance. Les boutons de provider en haut permettent de relancer la recherche sur une autre source sans fermer la fenêtre.

### 2 — Approuver ou rejeter

- **Approuver** : valide le lien et synchronise immédiatement tous les champs (description, couverture, auteurs, statut, liste des tomes)
- **Rejeter** : écarte ce résultat sans synchroniser

Vous pouvez approuver **plusieurs providers** pour une même série. Le premier lien approuvé devient automatiquement le **provider principal** de la série. Depuis la fenêtre de métadonnées, chaque provider lié est listé et vous pouvez basculer le principal via **Définir comme principal**.

### Provider principal et providers de secours

La synchronisation prend toujours les informations du provider **principal** en premier. Si un champ est absent chez le principal, Stripstream le complète en consultant les providers de secours (les autres liens approuvés), dans leur ordre d'approbation.

- Le principal est défini **par série** (pas au niveau de la bibliothèque).
- Si le principal est rejeté ou supprimé, le plus ancien lien approuvé restant est automatiquement promu.
- Les **genres** ne sont jamais complétés par un provider de secours : seul le principal peut les alimenter (l'IA et l'édition manuelle restent la source de vérité).
- Les notes de communauté agrègent tous les providers approuvés (voir [Notes](/series/ratings/)).

### 3 — Verrouiller des champs

Après synchronisation, vous pouvez modifier manuellement n'importe quel champ. Pour empêcher le prochain rafraîchissement de l'écraser, activez le **verrou** sur ce champ via l'icône cadenas.

![Modal d'édition d'une série — les cadenas oranges indiquent les champs verrouillés](/screenshots/series-edit-locked-fields.png)

### 4 — Rafraîchir

Le bouton **Refresh** re-télécharge les données du provider et met à jour les champs non verrouillés. Utile quand un nouveau tome est sorti et que le nombre total de volumes doit être actualisé.

---

## Matching en masse

Plutôt que de matcher série par série, utilisez le job **Batch metadata** depuis la page Tâches :

- Traite toutes les séries sans lien approuvé
- Valide automatiquement les matchs avec un score de confiance maximum
- Les autres résultats sont listés dans le rapport du job pour traitement manuel

Voir [Batch & Refresh](/metadata/batch-refresh/) pour le détail.

---

## Ce qui est synchronisé

### Sur la série

Lors de l'approbation, Stripstream met à jour : description, auteurs, éditeurs, année de début, nombre total de tomes, statut de publication, genres, couverture. Les champs manquants sont complétés par les providers de secours, sauf les **genres**, qui ne proviennent que du provider principal.

### Sur les livres de la série

Pour chaque tome de la série, Stripstream met à jour : résumé, ISBN, date de publication, langue, auteurs.

### Champs verrouillables

Sur une **série** : description, auteurs, éditeurs, année de début, nombre total de tomes, statut, genres.

Sur un **livre** : résumé, ISBN, date de publication, langue, auteurs.

:::important
Tous les champs verrouillés sont systématiquement ignorés par la synchronisation, quelle que soit la source. Le verrouillage est votre protection contre les mises à jour automatiques non souhaitées.
:::

---

## Comment les livres sont appariés

Quand Stripstream récupère la liste des tomes d'un provider, il les rapproche de vos livres locaux en deux ou trois étapes :

1. **Par numéro de volume** — si le provider indique "tome 5", Stripstream cherche votre tome 5
2. **Par titre** — si le numéro de volume ne suffit pas, il compare les titres (insensible à la casse)
3. **Par ISBN / EAN** — uniquement avec **BDTheque** et **BDphile**, et seulement si les deux étapes précédentes n'ont rien trouvé : Stripstream compare l'ISBN/EAN du provider à celui de vos livres (chiffres et `X` seuls). Ce rapprochement est **additif** : il ne déplace jamais un tome déjà apparié par numéro ou par titre.

Seuls les tomes réguliers et les intégrales participent à ce matching.

---

## Score de confiance

Le score (0.0 → 1.0) mesure la probabilité que le résultat trouvé corresponde bien à votre série. Il combine la similarité de nom avec un bonus si le nombre de tomes correspond.

Seul un score de **1.0** déclenche la validation automatique dans le job batch. En dessous, le match vous est soumis pour validation manuelle.

:::note[Détails techniques]
**Règles de mise à jour par champ** :

Série :

| Champ | Règle |
|-------|-------|
| `description` | Remplace si non-vide |
| `authors` | Remplace si tableau non-vide |
| `publishers` | Remplace si tableau non-vide |
| `start_year` | Remplace si absent en base |
| `total_volumes` | Remplace si absent en base |
| `status` | Remplace si absent en base (via mappings de statut) |
| `genres` | Remplace si tableau non-vide (provider principal uniquement) |
| `cover_url` | Remplace si non-vide |

Livres :

| Champ | Règle |
|-------|-------|
| `summary` | `COALESCE(NULLIF(new, ''), existing)` |
| `isbn` | Remplace si non-vide |
| `publish_date` | Remplace si non-vide |
| `language` | Remplace si non-vide |
| `authors` | Remplace si tableau non-vide |

**Score de confiance** : boost +0.30 si nombre de tomes identique (local == provider), +0.15 si proche (±2 tomes).

**Matching des livres** : volume 0 (HS chez certains providers) ignoré. Seuls `regular` et `integral` participent au matching.
:::
