---
title: Volumes manquants
description: Détection et affichage des volumes manquants dans une série
---

:::caution[Prérequis]
Les volumes manquants ne s'affichent que si la série a un **lien metadata approuvé**. Sans lien approuvé, Stripstream ne sait pas combien de tomes existent ni lesquels vous manquent. Voir [Synchronisation des métadonnées](/metadata/sync/) pour approuver un lien.
:::

## Calcul des manquants

Le nombre de volumes manquants est calculé comme suit :

```
manquants = max(total_volumes - nombre_de_livres_regular, 0)
```

### Règles

- Seuls les livres `volume_type = 'regular'` sont comptés — les HS, intégrales et oneshots sont exclus
- `total_volumes` vient du provider de métadonnées et est éditable manuellement sur la page de la série
- Si `total_volumes` est `NULL`, aucun manquant n'est calculé (affiché comme 0)
- Modifier `total_volumes` manuellement met à jour le comptage immédiatement

---

## Affichage sur la page série

Les volumes manquants s'affichent comme des cartes grisées (niveaux de gris) mélangées avec vos livres possédés, triés par numéro de volume. Chaque carte affiche :

- La couverture du provider (en niveaux de gris)
- Le titre et le numéro de volume
- Un badge indiquant que le tome est manquant

Un bouton **toggle** permet d'afficher ou masquer les volumes manquants. L'affichage est activé par défaut.

Les données de couverture et titre viennent de `external_book_metadata`, peuplé lors de la synchronisation des métadonnées.

---

## Volumes disponibles au téléchargement

Si les téléchargements sont configurés et qu'un volume manquant a été trouvé par le job `download_detection`, une section **Volumes disponibles** apparaît sur la page de la série avec les résultats Prowlarr pour ce volume.

Voir [Téléchargements](/downloads/overview/) pour le workflow complet.
