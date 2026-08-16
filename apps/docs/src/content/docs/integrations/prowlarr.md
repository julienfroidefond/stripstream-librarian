---
title: Prowlarr
description: Recherche de releases et détection automatique de volumes manquants
---

Prowlarr est un gestionnaire d'indexeurs de torrents. Stripstream l'utilise pour trouver automatiquement les releases disponibles correspondant aux volumes manquants de vos séries.

## Configuration

Dans **Settings → Download tools → Prowlarr** :
- **URL** : adresse de votre instance Prowlarr (ex. `http://prowlarr:9696`)
- **Clé API** : disponible dans Prowlarr → Settings → General → API Key

---

## Découverte des releases (onglet Prowlarr)

L'onglet **Prowlarr** de la page Découverte agrège les releases récentes disponibles par série. Le bouton "+" ouvre un assistant pour ajouter la série à votre bibliothèque — voir [Ajouter à la bibliothèque](/discovery/add-to-library/).

---

## Recherche manuelle depuis une série

Sur la page d'une série, le bouton **Prowlarr** lance une recherche ciblée. Les résultats sont triés par pertinence et indiquent quels volumes manquants sont couverts par chaque release. Vous pouvez envoyer directement un résultat à qBittorrent.

Lorsqu'un titre ou un numéro de tome est ambigu, Stripstream demande une confirmation avant l'envoi à qBittorrent. Vérifiez alors la release proposée avant de confirmer ; ce contrôle évite qu'une série ou un tome homonyme soit téléchargé par erreur.

---

## Détection automatique

Stripstream peut interroger automatiquement Prowlarr à intervalles réguliers pour toutes vos séries avec des volumes manquants. Les releases trouvées apparaissent dans la section **Disponibles au téléchargement** de la page Téléchargements.

La fréquence se configure dans les paramètres de chaque bibliothèque → section **Détection de téléchargements**.

---

## Polling RSS

En complément de la détection classique (une requête par série), le polling RSS récupère les sorties récentes de tous vos indexeurs en parallèle et les compare en mémoire à vos séries. Il est plus rapide et consomme moins de requêtes.

Configurez l'intervalle dans **Settings → Download tools → Prowlarr → Polling RSS automatique**.

:::caution
Certains indexeurs privés n'acceptent pas les requêtes RSS sans terme de recherche explicite. Ces indexeurs sont ignorés silencieusement par le polling RSS — leurs releases restent accessibles via la détection classique.
:::

---

## Gestion des releases

**Indicateur d'échec** : une release est marquée si un téléchargement précédent l'utilisant a échoué. Elle reste disponible mais vous êtes averti.

**Correspondance à vérifier** : certaines releases sont trouvées mais leur rapprochement avec la série ou le tome reste incertain. Elles restent visibles afin de ne pas masquer une bonne release, mais demandent confirmation avant téléchargement.

**Blacklist** : masquez définitivement les releases indésirables — elles ne seront plus proposées lors des prochaines détections. Pour gérer la blacklist, accédez au panel via l'icône œil à côté du titre "Disponibles au téléchargement" sur la page Téléchargements.

---

## Tri sur la page Téléchargements

La liste des volumes disponibles peut être triée par :

| Tri | Description |
|-----|-------------|
| **Récent** (défaut) | Releases détectées le plus récemment en premier |
| Seeders | Par nombre de seeders de la meilleure release |
| Manquants | Par nombre de volumes manquants |
| Nom | Alphabétique |

:::note[Détails techniques]
**Détection classique** : job `download_detection`. Stocke les résultats dans la table `available_downloads`. `detected_at` de chaque release est préservé entre les runs. Filtre les releases blacklistées (`release_blacklist`).

**Polling RSS** : job `prowlarr_rss`, global (`library_id = NULL`). Fetch parallèle via `/api/v1/search?query=&indexerIds=X&limit=100`. Déduplication par GUID. Normalisation des titres : points, underscores, tirets, apostrophes → espaces, accents supprimés, insensible à la casse. Snapshot RSS (max 200 releases) stocké dans `stats_json.rss_releases`, purgé automatiquement — 5 derniers jobs `success` conservés.

**Stratégie de fetch deux passes** (onglet Découverte) : passe 1 = requêtes par mots-clés genre triées par seeders ; passe 2 = requête vide triée par publishDate. Dédupliqué par GUID, mis en cache 7 jours, limite 200 résultats.

**Blacklist** : stockée dans `release_blacklist` (titre + indexer + nom de série). Ne supprime pas les entrées `available_downloads` existantes — nettoyage au prochain run.
:::
