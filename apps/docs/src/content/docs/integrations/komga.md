---
title: Komga
description: Import de la progression de lecture depuis un serveur Komga
---

L'intégration Komga permet d'importer votre historique de lecture depuis un serveur Komga existant. C'est une opération **ponctuelle** : Komga est interrogé à la demande, et les livres marqués comme lus dans Komga sont mis à jour dans Stripstream.

:::note
Il s'agit d'un import **unidirectionnel** : Komga → Stripstream uniquement. Stripstream ne modifie pas votre serveur Komga.
:::

## Configuration

Dans **Settings → onglet Komga** :

1. **URL** — adresse complète de votre serveur Komga (ex. `https://komga.example.com`)
2. **Nom d'utilisateur** — votre identifiant Komga
3. **Mot de passe** — votre mot de passe Komga (non sauvegardé, à re-saisir à chaque synchronisation)
4. **Utilisateur local** — sélectionnez quel utilisateur Stripstream reçoit la progression importée

---

## Synchroniser

Cliquez sur **Synchroniser les livres lus** pour lancer l'import. Stripstream récupère tous vos livres marqués lus dans Komga et met à jour leur statut de lecture dans votre bibliothèque locale.

---

## Rapport de synchronisation

Après chaque synchronisation, un rapport s'affiche :

| Compteur | Description |
|----------|-------------|
| **Lus sur Komga** | Nombre total de livres lus récupérés depuis Komga |
| **Matchés** | Livres Komga retrouvés dans votre bibliothèque locale |
| **Déjà lus** | Livres matchés qui étaient déjà marqués lus localement |
| **Nouvellement marqués** | Livres passés en "lu" lors de cette synchronisation |

Deux listes dépliables donnent le détail des livres matchés et non matchés. Les livres non matchés indiquent soit une différence de titre entre Komga et Stripstream, soit un livre non encore importé.

---

## Historique des synchronisations

Les 20 dernières synchronisations sont listées sous le formulaire. Cliquez sur une entrée pour revoir son rapport complet.

:::note[Détails techniques]
Le matching des livres se fait en deux passes :
1. **Primaire** : `(titre_série_normalisé, titre_livre_normalisé)` — correspondance exacte insensible à la casse
2. **Secondaire** : `titre_livre_normalisé` seul, si la passe primaire échoue

Les livres Komga sont récupérés paginés par 100 via l'API Komga.
:::
