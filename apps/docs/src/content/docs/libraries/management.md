---
title: Gestion des bibliothèques
description: Créer et gérer vos bibliothèques
---

## Multi-bibliothèques

Stripstream permet de créer et gérer plusieurs bibliothèques indépendantes, chacune avec son propre chemin racine.

- **Activer/désactiver** individuellement chaque bibliothèque
- **Supprimer** une bibliothèque cascade vers tous ses livres, jobs et métadonnées
- **Provider par défaut** configurable par bibliothèque (avec fallback global)

## Surveillance en temps réel

Deux mécanismes de détection automatique des changements :

| Mécanisme | Description | Configurable |
|-----------|-------------|-------------|
| **Scan périodique** | Intervalle configurable (défaut 5s) | `monitor_enabled` |
| **Watcher filesystem** | Détection temps réel des modifications | `watcher_enabled` |

Chaque mécanisme peut être activé/désactivé individuellement par bibliothèque.
