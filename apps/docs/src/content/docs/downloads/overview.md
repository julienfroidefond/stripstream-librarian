---
title: Téléchargements
description: Détecter les volumes manquants et les télécharger automatiquement via Prowlarr et qBittorrent
---

Le système de téléchargements permet de trouver automatiquement les volumes manquants de vos séries via Prowlarr, de les envoyer à qBittorrent, et de les importer dans votre bibliothèque une fois le téléchargement terminé — sans intervention manuelle.

## Activation

Les fonctionnalités de téléchargement sont désactivées par défaut. Pour les activer :

**Settings → Téléchargements → activer le toggle "Activer les téléchargements"**

Ce toggle contrôle la visibilité de la page Téléchargements dans le menu, les boutons d'envoi à qBittorrent, et les options de détection automatique dans les paramètres des bibliothèques.

## Prérequis

Avant d'utiliser les téléchargements, configurez les deux intégrations dans **Settings** :

| Intégration | Ce qu'il faut renseigner |
|-------------|--------------------------|
| [Prowlarr](/integrations/prowlarr) | URL + clé API |
| [qBittorrent](/integrations/qbittorrent) | URL + nom d'utilisateur + mot de passe |

Prowlarr sert à **trouver** les releases. qBittorrent sert à les **télécharger**.

## Workflow de bout en bout

```
Bibliothèque
    │
    ├─ Volumes manquants détectés
    │         │
    │         ▼
    │   Job "download_detection"
    │   (Prowlarr cherche les releases)
    │         │
    │         ▼
    │   Releases stockées dans
    │   "Téléchargements disponibles"
    │         │
    │         ▼ (clic sur le bouton ↓)
    │   Envoi à qBittorrent
    │         │
    │         ▼
    │   Téléchargement en cours
    │   (suivi en temps réel)
    │         │
    │         ▼
    │   Import automatique
    │   (fichiers copiés dans la bibliothèque)
    │         │
    │         ▼
    │   Scan + refresh métadonnées
    └─────────────────────────────────
```

### 1 — Détection

La détection est déclenchée par le job `download_detection`. Pour chaque série éligible de la bibliothèque, il interroge Prowlarr et stocke les releases pertinentes.

:::caution[Conditions pour qu'une série soit recherchée]
Une série n'est cherchée dans Prowlarr que si elle remplit **les deux conditions suivantes** :

1. **Lien metadata approuvé** — la série doit avoir été matchée à un provider de métadonnées et le lien doit être en statut *Approuvé*. Sans cela, Stripstream ne sait pas quels volumes existent.
2. **Volumes manquants** — le provider doit connaître des volumes que vous n'avez pas encore dans votre bibliothèque.

Une série sans lien metadata, ou dont vous possédez tous les volumes connus, est silencieusement ignorée.
:::

**Déclenchement** :
- **Automatique** : configurez une fréquence (horaire / quotidien / hebdomadaire) dans les paramètres de chaque bibliothèque → section *Détection de téléchargements*
- **Manuel** : depuis la page d'une série, cliquez sur le bouton Prowlarr pour lancer une recherche ciblée

### 2 — Téléchargements disponibles

Les releases trouvées apparaissent dans la section **Volumes disponibles** de la page Téléchargements. Pour chaque série :
- Nombre de volumes manquants
- Liste des releases avec indexer, seeders, taille, volumes couverts
- Bouton d'envoi à qBittorrent par release

### 3 — Suivi du téléchargement

Une fois envoyée à qBittorrent, la release apparaît dans la liste du haut avec son statut en temps réel.

### 4 — Import automatique

Quand qBittorrent termine le téléchargement, l'indexer détecte la complétion, importe les fichiers dans la bibliothèque (avec renommage si configuré), puis lance un scan et un refresh des métadonnées.

---

## Page Téléchargements

### Historique des téléchargements

Chaque téléchargement envoyé à qBittorrent apparaît dans la liste avec son statut :

| Statut | Description |
|--------|-------------|
| `En cours` | Téléchargement en progression (barre + vitesse + ETA) |
| `Terminé` | Téléchargé, en attente d'import |
| `Import en cours` | Fichiers en cours de copie dans la bibliothèque |
| `Importé` | Fichiers copiés avec succès |
| `Partiel` | Certains fichiers importés, d'autres skippés |
| `Aucun fichier importé` | Aucun fichier ne correspondait aux volumes attendus |
| `Erreur` | Import échoué |

**Filtres** : Tous · Actifs · Importés · Erreur

**Actions disponibles** :
- **Retry** : relancer l'import pour les statuts `Erreur`, `Partiel`, `Aucun fichier importé`
- **Annuler** : interrompre un téléchargement en cours dans qBittorrent
- **Supprimer** : retirer l'entrée de l'historique

### Volumes disponibles

En bas de la page, la liste des releases détectées mais pas encore demandées au téléchargement.

**Tri** :

| Option | Description |
|--------|-------------|
| Récent (défaut) | Par date de détection, les plus récentes en premier |
| Seeders | Par nombre de seeders de la meilleure release |
| Manquants | Par nombre de volumes manquants |
| Nom | Alphabétique |

**Filtre bibliothèque** : si vous avez plusieurs bibliothèques, un sélecteur permet de n'afficher qu'une bibliothèque.

Cliquez sur une ligne pour déplier les releases disponibles pour cette série. Pour chaque release :
- Volumes couverts (ex. `T03`, `T04-T06`)
- Indexer + seeders + taille
- Bouton d'envoi à qBittorrent
- Bouton de blacklist (masquer définitivement cette release)
- Bouton de suppression

**Tout ignorer** : supprime toutes les releases disponibles d'une série sans les blacklister.

---

## Recherche manuelle depuis une série

Sur la page d'une série, le bouton **Prowlarr** ouvre une recherche ciblée dans Prowlarr pour cette série. Les résultats sont triés par pertinence et affichent les volumes manquants matchés. Vous pouvez envoyer directement un résultat à qBittorrent depuis cette fenêtre.

---

## Blacklist

La blacklist masque une release définitivement — elle ne sera plus proposée lors des prochaines détections automatiques.

Pour gérer la blacklist : page Téléchargements → icône œil à côté du titre "Volumes disponibles". Le panel affiche toutes les releases blacklistées avec la possibilité de les réafficher.

---

## Mode replace

Lors de l'envoi à qBittorrent, une option **Replace** importe tous les fichiers du torrent, sans se limiter aux volumes manquants attendus. Utile pour remplacer des fichiers existants par une version de meilleure qualité.
