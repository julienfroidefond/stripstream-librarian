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
| **Lien métadonnées** | Lien vers la fiche externe du lien approuvé |
| **Âge** | Nombre de jours depuis la création de la série |

Un badge **Sortie wishlist détectée** signale qu'une sortie correspondant à la série attend dans la
wishlist de détection. Ce n'est **pas** un téléchargement en cours : rien n'est en train d'être
téléchargé. Supprimer la série retire simplement cette entrée de la wishlist, c'est pourquoi la
suppression est déconseillée tant que la détection est utile.

---

## Supprimer une série orpheline

- **Suppression unitaire** — bouton *Supprimer* sur la ligne, puis confirmation dans la modale.
- **Suppression de la page** — bouton *Supprimer la sélection de cette page* en haut du tableau, qui
  supprime uniquement les séries affichées sur la page courante (pas les autres pages).

La suppression passe par l'endpoint dédié `DELETE /admin/series/orphans/{series_id}`. Dans une même
transaction, l'API verrouille la ligne de la série (`FOR UPDATE`) puis vérifie qu'elle n'a toujours
aucun livre. Si un livre a été importé entre l'affichage de la liste et la suppression, l'API répond
`409` et ne supprime rien. La série est ensuite **archivée** avant d'être retirée, ses liens de
métadonnées externes et ses entrées de wishlist associées sont supprimés en cascade. Comme aucun
livre n'existe, aucun fichier n'est touché sur le disque.

---

:::note[API]
L'onglet s'appuie sur `GET /admin/series/orphans` (route admin, paginée, triée par date de création
croissante) et sur `DELETE /admin/series/orphans/{series_id}` pour chaque suppression. Le backoffice
expose le tout via `/api/series/orphans` : le corps doit contenir soit `{"ids": ["..."]}`, soit
explicitement `{"all": true}` ; toute autre valeur est refusée en `400`.
:::
