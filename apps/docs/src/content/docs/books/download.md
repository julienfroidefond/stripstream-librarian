---
title: Télécharger un livre
description: Récupérer le fichier original d'un livre depuis sa fiche
---

![Fiche d'un livre avec le menu d'actions ouvert](/screenshots/book-detail.png)

## Télécharger le fichier original

Depuis la fiche d'un livre, ouvrez le menu **Plus**, section **Actions**, puis cliquez sur **Télécharger le fichier original**.

Le fichier est envoyé tel quel : c'est exactement celui présent sur le disque, avec son nom actuel (renommage inclus), sans conversion ni retraitement.

:::tip
Il s'agit d'un vrai lien de téléchargement : le clic droit → **Enregistrer la cible du lien sous…** fonctionne aussi.
:::

:::note[iPhone / iPad — application installée]
Dans l'application installée sur l'écran d'accueil (mode PWA), iOS ne peut pas télécharger un fichier par simple navigation : le WebView reste bloqué sur une page vide, sans bouton retour. Stripstream détecte ce cas et remet le fichier à la **feuille de partage iOS** : choisissez **Enregistrer dans Fichiers** (ou l'application de lecture de votre choix). Dans un onglet Safari classique, le téléchargement direct fonctionne normalement.
:::

---

## Formats et types MIME

| Format | Type renvoyé |
|--------|--------------|
| CBZ | `application/vnd.comicbook+zip` |
| CBR | `application/vnd.comicbook-rar` |
| PDF | `application/pdf` |
| EPUB | `application/epub+zip` |

---

:::note[Détails techniques]
Le backoffice relaie `GET /api/books/{id}/file`. Côté API Rust, le fichier est résolu via la dernière entrée `book_files` du livre, remappé depuis `/libraries`, puis **streamé** (`Body::from_stream`, pas de chargement complet en mémoire). Le nom est transmis dans `Content-Disposition` avec un repli ASCII et la forme UTF-8 (`filename*=UTF-8''…`) pour les noms accentués.

Limites : pas de requêtes `Range` (téléchargement en un seul flux, pas de reprise) — l'endpoint est sur les routes de lecture (scope `read`).
:::
