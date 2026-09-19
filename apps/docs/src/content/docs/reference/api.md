---
title: API
description: Référence de l'API REST
---

## Documentation interactive

L'API expose deux interfaces Swagger UI accessibles depuis votre navigateur :

| Interface | URL | Accès |
|-----------|-----|-------|
| **Client API** | `/swagger-ui` | Endpoints de lecture (lecture seule) |
| **Admin API** | `/admin/swagger-ui` | Tous les endpoints |

Un menu déroulant permet de basculer entre les deux spécifications.

## Endpoints publics (sans authentification)

- `GET /health` — État du service
- `GET /ready` — Disponibilité
- `GET /metrics` — Métriques Prometheus
- `GET /swagger-ui` — Documentation Swagger

## Endpoints de lecture (token read)

Accessibles avec un token de scope `read` :

- Bibliothèques, livres, séries, auteurs — listing et détail
- Pages de livres et miniatures
- Progression de lecture (lecture et mise à jour)
- Recherche full-text, statistiques
- Métadonnées : liens (`GET /metadata/links?series_id=...`), volumes manquants avec couvertures (`GET /metadata/missing/{link_id}`) et liste des providers (`GET /metadata/providers`)

## Endpoints admin (token admin)

Accessibles avec un token de scope `admin` :

- CRUD bibliothèques et configuration
- Édition des métadonnées (livres et séries)
- Conversion CBR, renommage
- Gestion des tâches (déclenchement, annulation, flux SSE)
- Gestion des tokens API
- Opérations metadata (recherche, match, approbation, rejet, batch, refresh)
- Intégrations externes (Prowlarr, qBittorrent, Komga)
- Paramètres application et gestion du cache

## Authentification

Passez le token dans le header `Authorization` :

```bash
curl -H "Authorization: Bearer stl_abc_votre_token_complet" \
  http://localhost:7080/api/series
```

:::note[Détails techniques]
**Bootstrap token** : token admin initial via la variable d'environnement `API_BOOTSTRAP_TOKEN`. Utilisé uniquement pour créer les premiers tokens API depuis l'interface.

**Tokens API** : format `stl_{prefix}_{secret}`, hash Argon2 en base de données. Deux scopes : `admin` (accès complet) et `read` (consultation, progression de lecture, favoris et notations personnelles). Un token `read` peut donc ajouter ou supprimer ses propres favoris et modifier ou supprimer ses propres notations.

**Rate limiting** : fenêtre glissante configurable, défaut 120 req/s.

**Dual spec OpenAPI** : Client API (`/openapi.json`, scope read) et Admin API (`/admin/openapi.json`, tous scopes). La Client API inclut notamment les endpoints de progression, favoris et notations associés à l'utilisateur du token.
:::
