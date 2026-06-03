---
title: Volumes manquants
description: Détection et affichage des volumes manquants dans une série
---

:::caution[Prérequis]
Les volumes manquants ne s'affichent que si la série a des **métadonnées approuvées**. Sans cela, Stripstream ne sait pas combien de tomes existent ni lesquels vous manquent. Voir [Synchronisation des métadonnées](/metadata/sync/) pour approuver un lien.
:::

## Ce que vous voyez

Sur la page d'une série, les tomes que vous ne possédez pas apparaissent comme des cartes grisées (en niveaux de gris), mélangées avec vos livres possédés dans l'ordre des volumes. Chaque carte affiche :

- La couverture du tome (en niveaux de gris)
- Le titre et le numéro de volume
- Un badge indiquant que le tome est manquant

Un bouton **toggle** permet d'afficher ou masquer les volumes manquants.

## Comment le nombre de manquants est calculé

Stripstream compare le nombre de tomes que vous possédez avec le nombre total de tomes annoncé par le provider de métadonnées.

- Le nombre total de tomes est affiché sur la page de la série et peut être modifié manuellement
- Seuls les **tomes réguliers** sont comptés — les hors-séries, oneshots et intégrales sont exclus
- Si le nombre total de tomes n'est pas renseigné (provider qui ne donne pas l'information), aucun manquant n'est calculé

## Volumes disponibles au téléchargement

Si les téléchargements sont configurés et qu'un volume manquant a été trouvé via Prowlarr, une section **Volumes disponibles** apparaît sur la page de la série avec les releases disponibles pour ce volume.

Voir [Téléchargements](/downloads/overview/) pour le workflow complet.

:::note[Détails techniques]
Calcul des manquants : `max(total_volumes - nombre_de_livres_regular, 0)`.

Seuls les livres avec `volume_type = 'regular'` sont comptés. `total_volumes` vient du champ éponyme du provider, stocké dans `metadata_json`. Si `total_volumes IS NULL`, le calcul retourne 0.

Les couvertures et titres des volumes manquants viennent de la table `external_book_metadata`, peuplée lors de la synchronisation des métadonnées.
:::
