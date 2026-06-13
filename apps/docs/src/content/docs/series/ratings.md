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

À l'inverse, lors d'un **Pull AniList** (`/anilist/pull`), votre score AniList existant est importé dans Stripstream si vous n'avez pas encore de note locale pour cette série (sans écraser une note existante).

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

## API

| Méthode | Chemin | Description | Scope |
|---|---|---|---|
| `GET` | `/series/:id/ratings` | Note locale, note AniList importée, notes providers | `read` |
| `PUT` | `/series/:id/rating` | Sauvegarder votre note (`{ "rating": 1–10 }`) | `admin` |
| `DELETE` | `/series/:id/rating` | Supprimer votre note | `admin` |
