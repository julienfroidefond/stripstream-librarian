---
title: Supprimer un livre
description: Retirer un livre de la bibliothèque et supprimer définitivement son fichier du disque
---

![Fiche d'un livre avec le menu d'actions ouvert](/screenshots/book-detail.png)

## Comment supprimer un livre

Depuis la fiche du livre, ouvrez le menu **Plus**, section **Actions**, puis cliquez sur **Supprimer** (🗑).

Un modal de confirmation rappelle que le fichier sera définitivement supprimé du disque et que l'action est irréversible. Validez avec **Supprimer**, ou **Annuler** pour abandonner.

:::caution
Le fichier physique (CBZ, CBR, PDF…) est effacé du disque. La suppression n'est pas annulable depuis le backoffice : il faut re-télécharger ou re-copier le fichier pour le retrouver.
:::

Une fois le livre supprimé, vous êtes ramené sur la **page de la série** à laquelle il appartenait. Si le livre n'appartient à aucune série, vous revenez sur la liste des séries.

---

## Ce qui est supprimé

| Élément | Sort |
|---------|------|
| Fichier physique (CBZ, CBR, PDF…) | ✗ supprimé du disque |
| Miniature (WebP) | ✗ supprimée du disque |
| Ligne du livre en base | ✗ supprimée |
| Progression de lecture (tous les utilisateurs) | ✗ supprimée |
| Métadonnées externes liées au livre | ✓ conservées, détachées du livre |
| Liens Telegram Monitor | ✓ conservés, détachés du livre |

---

## Si c'est le dernier livre de la série

La série est retirée immédiatement, sans attendre un scan. Avant sa suppression, elle est archivée : ses métadonnées et la progression de lecture de ses livres sont conservées et pourront être restaurées si la série revient. Voir [Archives](/series/archives/).

Exception : si la série a des métadonnées approuvées ou des volumes dans votre wishlist, elle est conservée même sans aucun livre.

---

## La suppression est immédiate

Aucun job d'indexation n'est déclenché : la suppression est appliquée directement en base et sur le disque, et le résultat est visible sans attendre le prochain [scan](/jobs/indexation/).

---

:::note[Détails techniques]
Le backoffice relaie la suppression vers `DELETE /api/books/{id}`. Le handler efface le fichier et sa miniature, puis supprime la ligne `books` — les tables liées suivent leurs contraintes : `book_files` et `book_reading_progress` en `ON DELETE CASCADE` (la progression de tous les utilisateurs est donc effacée), `external_book_metadata` et `telegram_book_links` en `ON DELETE SET NULL`.

Si c'est le dernier livre, `prune_series_if_empty` archive puis supprime la série, sauf si elle possède encore des liens de métadonnées externes ou des téléchargements disponibles (wishlist).
:::
