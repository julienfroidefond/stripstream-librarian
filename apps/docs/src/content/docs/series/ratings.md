---
title: Notation des séries
description: Notez vos séries, consultez les notes de la communauté et synchronisez avec AniList.
---

Stripstream Librarian propose un système de notation à trois niveaux : votre note personnelle, votre note importée depuis AniList, et les notes de la communauté récupérées depuis les providers de métadonnées.

## Notation locale (votre note)

Sur la page de détail d'une série, un widget **étoiles** vous permet de noter la série de **½ étoile à 5 étoiles** (stocké en interne sur une échelle 1–10).

- Survolez les étoiles pour prévisualiser la note.
- Cliquez sur une demi-étoile ou une étoile pleine pour valider.
- Cliquez sur **×** à côté des étoiles pour supprimer votre note.

La note est enregistrée par utilisateur — plusieurs comptes peuvent avoir des notes différentes sur la même série.

## Synchronisation avec AniList

Si la série est liée à AniList et que votre compte AniList est connecté (voir [Intégration AniList](/integrations/anilist/)), votre note locale est **automatiquement poussée vers AniList** dès que vous la modifiez.

La conversion est la suivante :

| Note locale (1–10) | Score AniList (POINT_100) |
|---|---|
| 1 (½ étoile) | 10 |
| 5 (2,5 étoiles) | 50 |
| 10 (5 étoiles) | 100 |

À l'inverse, le job **Import des notes AniList** (`rating_pull`) importe votre score AniList existant dans Stripstream pour toutes les séries déjà liées, sans écraser une note locale existante (voir [Tâches AniList](/jobs/anilist/)).

> **Note :** la suppression d'une note locale ne supprime pas votre score côté AniList pour éviter les pertes accidentelles.

## Notes de la communauté (providers)

Sous votre note se trouvent les notes moyennes remontées par les providers de métadonnées, affichées côte à côte :

| Provider | Échelle native | Exemples |
|---|---|---|
| AniList | 0–100 | `averageScore` |
| SensCritique | 0–10 | `rating` |
| Google Books | 0–5 | `averageRating` |
| OpenLibrary | 0–5 | `ratings_average` |

Ces notes sont mises à jour à chaque synchronisation ou rafraîchissement de métadonnées. Le nombre de votes est affiché entre parenthèses quand il est disponible.

## Notes communauté dans la liste des séries

La **note communauté** (moyenne normalisée sur 5 des notes providers) est disponible dans la page **Séries** :

- **Tri "Note communauté"** — classe les séries par score communauté décroissant. Les séries sans note apparaissent en fin de liste.
- **Groupes par note** — quand ce tri est actif, des séparateurs ★★★★★ / ★★★★ / ★★★ / ★★ / ★ / Sans note regroupent les séries par note entière.
- **Filtre "Mes notes"** (visible uniquement si un utilisateur est sélectionné) — trois valeurs : *Toutes*, *Notées uniquement*, *Non notées*.

:::note
La note communauté dans la liste est calculée côté API à partir des ratings providers approuvés. Elle se met à jour après un job **Refresh metadata**.
:::

## API

| Méthode | Chemin | Description | Scope |
|---|---|---|---|
| `GET` | `/series/:id/ratings` | Note locale, note AniList importée, notes providers | `read` |
| `PUT` | `/series/:id/rating` | Sauvegarder votre note (`{ "rating": 1–10 }`) | `admin` |
| `DELETE` | `/series/:id/rating` | Supprimer votre note | `admin` |
