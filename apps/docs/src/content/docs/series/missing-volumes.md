---
title: Volumes manquants
description: Détection et affichage des volumes manquants
---

## Calcul

Le nombre de volumes manquants est calculé ainsi :

```
missing = max(total_volumes - regular_book_count, 0)
```

### Règles

- Seuls les livres `volume_type = 'regular'` sont comptés
- Les HS, intégrales et oneshot sont **exclus** du calcul
- Basé sur `series.total_volumes` (éditable par l'utilisateur), pas le comptage externe
- `total_volumes = NULL` → 0 manquants
- La modification manuelle de `total_volumes` met à jour immédiatement le comptage

## Affichage

- Bouton toggle pour afficher/masquer les volumes manquants (affiché par défaut)
- Volumes manquants affichés comme cartes grisées avec couverture du provider (niveaux de gris)
- Données issues de `external_book_metadata` (cover_url, title, volume_number)
- Fonctionne aussi pour les séries découvertes sans livres locaux
- Fusionné avec les livres possédés et trié par numéro de volume
