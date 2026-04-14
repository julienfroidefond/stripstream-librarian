---
title: Synchronisation des métadonnées
description: Comment les métadonnées sont synchronisées entre providers et base locale
---

## Workflow de matching

1. **Recherche** : interroger un provider, obtenir des candidats avec scores de confiance
2. **Match** : lier une série à un résultat externe (statut `pending`)
3. **Approbation** : valider et synchroniser les métadonnées
4. **Rejet** : écarter un match

## Logique de synchronisation partagée

Les trois chemins de sync (approve, batch auto-match, refresh) utilisent le **même code factorisé** (`shared_sync.rs`).

### Champs synchronisés — Série

| Champ | Source | Règle de mise à jour |
|-------|--------|---------------------|
| `description` | `metadata_json.description` | Remplace si non-vide |
| `authors` | metadata_json ou candidat | Remplace si tableau non-vide |
| `publishers` | metadata_json ou candidat | Remplace si tableau non-vide |
| `start_year` | metadata_json ou candidat | `COALESCE(new, existing)` |
| `total_volumes` | total_volumes_external ou candidat | `COALESCE(new, existing)` |
| `status` | metadata_json (normalisé via `status_mappings`) | `COALESCE(new, existing)` |
| `genres` | metadata_json | Remplace si tableau non-vide |
| `cover_url` | metadata_json ou candidat | Remplace si non-vide |

:::important
Tous les champs respectent le **verrouillage** (`locked_fields`). Si un champ est verrouillé, la synchronisation ne le modifie pas.
:::

La série est créée si absente (`INSERT ... ON CONFLICT DO UPDATE`), ce qui permet la création depuis la découverte.

### Champs synchronisés — Livres

| Champ | Règle de mise à jour |
|-------|---------------------|
| `summary` | Remplace si non-vide |
| `isbn` | Remplace si non-vide |
| `publish_date` | Remplace si non-vide |
| `language` | Remplace si non-vide |
| `authors` / `author` | Remplace si tableau non-vide |

Tous les champs livres respectent aussi le verrouillage.

### Matching des livres

Les livres externes sont appariés aux livres locaux en deux étapes :

1. **Par numéro de volume** : correspondance exacte (skip volume 0 = HS chez les providers)
2. **Par titre** : containment case-insensitive si le volume n'a pas matché

Seuls les livres `volume_type IN ('regular', 'integral')` participent au matching.

## Scoring de confiance

### Niveau provider
- Similarité de nom (Jaccard/containment normalisé)
- Bonus pour les éditions avec plus de volumes

### Boost par le nombre de livres locaux
| Condition | Boost |
|-----------|-------|
| Exact match (local == total_volumes) | +0.30 |
| Proche (±2 volumes) | +0.15 |

- Candidats retriés par confiance après boost
- Seuil d'auto-match : confiance == 1.0

## Verrouillage de champs

- Champs série verrouillables : `description`, `authors`, `publishers`, `start_year`, `total_volumes`, `status`
- Champs livre verrouillables : `summary`, `isbn`, `publish_date`, `language`, `authors`
- Stocké en JSONB : `{"description": true, "authors": true}`
- Les rapports de sync distinguent les champs mis à jour vs ignorés (verrouillés)
