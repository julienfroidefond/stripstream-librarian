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
- Les **tomes réguliers** et les **intégrales numérotées** sont comptés ; les hors-séries et oneshots sont exclus
- Une **intégrale sans numéro de tome** (ex. `Dragon Ball INT.cbz`) marque la série comme complète : aucun volume manquant n'est affiché
- Une **intégrale numérotée** (ex. `Dragon Ball INT 2.cbz`) ne couvre que son tome : elle compte comme un volume possédé, les autres restent manquants
- Si le nombre total de tomes n'est pas renseigné (provider qui ne donne pas l'information), aucun manquant n'est calculé

## Volumes disponibles au téléchargement

Si les téléchargements sont configurés et qu'un volume manquant a été trouvé via Prowlarr, une section **Volumes disponibles** apparaît sur la page de la série avec les releases disponibles pour ce volume.

Voir [Téléchargements](/downloads/overview/) pour le workflow complet.

:::note[Détails techniques]
Calcul des manquants :
- `0` si la série contient au moins une intégrale **sans numéro de tome** (`volume_type = 'integral' AND volume IS NULL`) ;
- sinon `max(total_volumes - volumes_possédés, 0)`, où `volumes_possédés` compte les numéros distincts des livres `regular` et `integral`, plus les livres `regular`/`integral` sans numéro.

`total_volumes` vient du champ éponyme du provider, stocké dans `metadata_json`. Si `total_volumes IS NULL`, le calcul retourne 0.

:::caution[Écart connu]
La détection de téléchargements applique une règle plus large : elle considère la série comme complète dès qu'elle contient **une intégrale, numérotée ou non** (voir [Tâches de détection](/jobs/telechargements/)). Une série avec une intégrale numérotée peut donc afficher des volumes manquants tout en étant ignorée par la détection.
:::

Les couvertures et titres des volumes manquants viennent de la table `external_book_metadata`, peuplée lors de la synchronisation des métadonnées.
:::
