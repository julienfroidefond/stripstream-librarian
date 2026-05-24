---
title: Utilisateurs & Tokens
description: Gérer les utilisateurs et les tokens d'API
---

La gestion des utilisateurs et des tokens se fait dans **Settings → onglet Jetons**.

![Page utilisateurs et tokens API — liste des utilisateurs, création de token avec portée et utilisateur associé](/screenshots/users-tokens-page.png)

## Utilisateurs

Les utilisateurs servent à suivre la progression de lecture de façon indépendante. Chaque utilisateur a ses propres statuts de lecture (lu / en cours / non lu) sur chaque livre.

### Compte administrateur

Il existe toujours un compte admin défini par les variables d'environnement `ADMIN_USERNAME` et `ADMIN_PASSWORD`. Ce compte n'est pas modifiable depuis l'interface et a toujours le scope `admin`.

### Créer un utilisateur lecteur

Dans Settings → Tokens → section **Lecteurs**, entrez un nom d'utilisateur et validez. L'utilisateur est créé immédiatement.

Les utilisateurs lecteurs :
- Ont leur propre suivi de lecture (livres lus, en cours, non lus)
- Apparaissent dans les filtres de la page série et sur le dashboard
- Peuvent recevoir un ou plusieurs tokens API de scope `read`
- Ne peuvent pas accéder aux fonctions admin

### Renommer un utilisateur

Cliquez directement sur le nom dans le tableau pour le modifier inline.

### Supprimer un utilisateur

Le bouton supprimer retire l'utilisateur et ses données de lecture. Cette action est irréversible.

---

## Tokens API

Les tokens permettent d'accéder à l'API sans passer par le backoffice. Ils sont utiles pour les applications tierces, l'automatisation, ou les lecteurs externes.

### Scopes

| Scope | Accès |
|-------|-------|
| `read` | Lecture seule — bibliothèques, livres, séries, progression, pages |
| `admin` | Accès complet — toutes les opérations d'administration |

### Créer un token

Dans Settings → Tokens → section **Tokens API** :
1. Donnez un nom descriptif au token (ex. `Koreader`, `Script backup`)
2. Choisissez le scope (`read` ou `admin`)
3. Associez optionnellement un utilisateur lecteur — le token portera son contexte de lecture
4. Validez : **le token est affiché une seule fois**, copiez-le immédiatement

### Format du token

```
stl_<prefix>_<secret>
```

Le `prefix` est visible dans la liste des tokens pour l'identifier sans exposer le secret. Le secret est stocké hashé en base (Argon2) — le backoffice ne peut plus le relire.

### Utiliser un token

Passez le token dans le header `Authorization` :

```bash
curl -H "Authorization: Bearer stl_abc_votre_token_complet" \
  http://localhost:7080/api/series
```

### Révoquer vs supprimer

- **Révoquer** : invalide le token immédiatement mais le conserve dans l'historique
- **Supprimer** : retire définitivement le token révoqué de la liste

Un token actif ne peut être que révoqué, pas directement supprimé.

### Réassigner un token

Le sélecteur dans la colonne **Utilisateur** du tableau permet de changer l'utilisateur associé à un token sans le révoquer.

---

## Multi-utilisateurs et lecture

Le modèle multi-utilisateurs est pensé pour les foyers ou groupes qui partagent une même instance mais veulent une progression de lecture séparée.

![Sélecteur d'utilisateur dans le backoffice — bascule entre Admin et les utilisateurs lecteurs](/screenshots/user-selector.png)

Concrètement :
- Sur la page d'une série, un filtre par lecteur permet de voir les statuts de chaque utilisateur
- Sur le dashboard, les graphiques de lecture peuvent être filtrés par lecteur
- La synchronisation AniList et Komga est associée à un utilisateur spécifique (configurable dans les settings de ces intégrations)
