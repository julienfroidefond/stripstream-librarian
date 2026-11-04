---
title: Séries orphelines
description: Repérer et supprimer les séries de découverte restées sans aucun livre
---

Une série **orpheline** est une série présente en base mais qui ne possède aucun livre. Ce sont le
plus souvent des restes de découverte ou d'import : une série ajoutée depuis un provider de
métadonnées (ou via un téléchargement) dont les fichiers n'ont jamais été importés.

---

## Consulter les séries orphelines

L'onglet **Paramètres → Séries orphelines** liste les séries sans aucun livre, de la plus ancienne
à la plus récente. Chaque ligne affiche :

| Colonne | Contenu |
|---------|---------|
| **Nom** | Nom de la série |
| **Bibliothèque** | Bibliothèque de rattachement |
| **Provider** | Provider du lien de métadonnées approuvé, si présent |
| **Lien metadata** | Lien vers la fiche externe du lien approuvé |
| **Âge** | Nombre de jours depuis la création de la série |

Un badge **Téléchargement en cours** signale une série qui a un téléchargement en attente dans la
liste de souhaits. La suppression reste possible mais est déconseillée : le téléchargement en cours
serait perdu.

---

## Supprimer une série orpheline

- **Suppression unitaire** — bouton *Supprimer* sur la ligne, puis confirmation dans la modale.
- **Suppression en masse** — bouton *Tout supprimer* en haut du tableau, qui supprime toutes les
  séries actuellement listées.

La suppression passe par le même chemin que la suppression manuelle d'une série : la série est
**archivée** avant d'être retirée, ses liens de métadonnées externes et ses téléchargements
associés sont supprimés en cascade. Aucun livre n'existe pour ces séries, donc aucun fichier n'est
touché sur le disque.

---

:::note[API]
L'onglet s'appuie sur `GET /admin/series/orphans` (route admin, paginée, triée par date de création
croissante) et sur `DELETE /series/{series_id}` pour chaque suppression. Le backoffice expose le
tout via `/api/series/orphans`.
:::
