---
title: API
description: Référence de l'API REST
---

## Documentation interactive

L'API expose deux spécifications OpenAPI avec Swagger UI :

| Spec | URL | Scope |
|------|-----|-------|
| **Client API** | `/swagger-ui` | Endpoints read |
| **Admin API** | `/admin/swagger-ui` | Tous les endpoints |

Dropdown pour basculer entre les specs.

## Endpoints publics (sans auth)

- `GET /health` — Health check
- `GET /ready` — Readiness
- `GET /metrics` — Métriques Prometheus
- `GET /swagger-ui` — Documentation Swagger

## Endpoints lecture (scope `read`)

- Bibliothèques, livres, séries, auteurs — listing et détail
- Pages de livres et miniatures
- Progression de lecture (get/update)
- Recherche full-text, statistiques
- Metadata links (`GET /metadata/links?series_id=...`)
- Livres manquants avec covers (`GET /metadata/missing/{link_id}`)

## Endpoints admin (scope `admin`)

- CRUD bibliothèques et configuration
- Édition metadata livres, conversion CBR
- Édition metadata séries
- Gestion des jobs (trigger, cancel, stream SSE)
- Gestion tokens API
- Opérations metadata (search, match, approve, reject, batch, refresh)
- Intégrations externes (Prowlarr, qBittorrent, Komga)
- Paramètres application et gestion du cache

## Authentification

- **Bootstrap token** : token admin via variable `API_BOOTSTRAP_TOKEN`
- **Tokens API** : format `stl_{prefix}_{secret}`, hash Argon2 en DB
- Deux scopes : `admin` (accès complet) et `read` (lecture seule)
- Rate limiting : fenêtre glissante configurable (défaut 120 req/s)
