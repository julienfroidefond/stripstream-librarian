---
title: Komga
description: Import de la progression de lecture depuis un serveur Komga
---

L'intégration Komga permet d'importer votre historique de lecture depuis un serveur Komga existant. C'est une opération **one-shot** : Komga est interrogé à la demande, et les livres marqués `READ` dans Komga sont mis à jour dans Stripstream.

:::note
Il s'agit d'un import **unidirectionnel** : Komga → Stripstream uniquement. Stripstream ne modifie pas votre serveur Komga.
:::

## Configuration

Dans **Settings → onglet Komga** :

1. **URL** — adresse complète de votre serveur Komga (ex. `https://komga.example.com`)
2. **Nom d'utilisateur** — votre identifiant Komga
3. **Mot de passe** — votre mot de passe Komga (non sauvegardé, uniquement utilisé lors de la synchronisation)
4. **Utilisateur local** — sélectionnez quel utilisateur Stripstream reçoit la progression importée

L'URL et le nom d'utilisateur sont mémorisés pour la prochaine synchronisation. Le mot de passe doit être re-saisi à chaque fois.

---

## Synchronisation

Cliquez sur **Synchroniser les livres lus** pour lancer l'import.

Le processus :
1. Récupère tous les livres au statut `READ` depuis l'API Komga (paginé par 100)
2. Tente de faire correspondre chaque livre Komga à un livre local par `(titre_série, titre_livre)` puis par `titre_livre` seul (insensible à la casse)
3. Met à jour le statut de lecture des livres matchés en `read` pour l'utilisateur local sélectionné

---

## Rapport de synchronisation

Après chaque synchronisation, un rapport s'affiche avec quatre compteurs :

| Compteur | Description |
|----------|-------------|
| **Lus sur Komga** | Nombre total de livres `READ` récupérés depuis Komga |
| **Matchés** | Livres Komga retrouvés dans votre bibliothèque locale |
| **Déjà lus** | Livres matchés qui étaient déjà marqués `read` localement |
| **Nouvellement marqués** | Livres passés en `read` lors de cette synchronisation |

Deux listes dépliables donnent le détail :
- **Livres matchés** — tous les livres trouvés (les nouvellement marqués sont mis en avant avec une coche)
- **Livres non matchés** — livres Komga pour lesquels aucun équivalent local n'a été trouvé

Les livres non matchés indiquent soit une différence de titre entre Komga et Stripstream, soit un livre non encore importé dans votre bibliothèque.

---

## Historique des synchronisations

Les 20 dernières synchronisations sont listées sous le formulaire. Cliquez sur une entrée pour revoir son rapport complet (compteurs + détail des livres matchés/non matchés).

---

## Matching des livres

Le matching se fait en deux passes :

1. **Primaire** : `(titre_série_normalisé, titre_livre_normalisé)` — correspondance exacte insensible à la casse
2. **Secondaire** : `titre_livre_normalisé` seul, si la passe primaire échoue

Si un titre Komga ne matche pas, vérifiez que la série est bien indexée dans Stripstream et que les titres sont identiques (ou proches) entre les deux systèmes.
