---
title: Gestion des séries
description: Organisation et métadonnées des séries
---

![Page détail d'une série — couverture, métadonnées, progression de lecture et grille de livres](/screenshots/series-detail.png)

## Comment les séries sont créées

Les séries sont créées automatiquement pendant le scan, à partir de la structure de dossiers. Chaque dossier contenant des livres devient une série. Les livres sans dossier parent sont regroupés sous le nom "unclassified".

## Structure de dossiers recommandée

Voici un exemple complet couvrant tous les cas reconnus par le scanner :

```
Ma Bibliothèque/                         ← racine de la bibliothèque
│
├── Dragon Ball/
│   ├── Dragon Ball T01.cbz              → série "Dragon Ball", tome 1,    régulier
│   ├── Dragon Ball T02.cbz              → série "Dragon Ball", tome 2,    régulier
│   ├── Dragon Ball HS1.cbz              → série "Dragon Ball", HS 1,      hors-série
│   ├── Dragon Ball INT.cbz              → série "Dragon Ball", intégrale,  intégrale
│   ├── Hors-Série/                      ← sous-dossier spécial → remonte au parent
│   │   └── Dragon Ball HS2.cbz          → série "Dragon Ball", HS 2,      hors-série
│   └── Intégrales/                      ← sous-dossier spécial → remonte au parent
│       └── Dragon Ball INT 2.cbz        → série "Dragon Ball",            intégrale
│
├── Naruto/
│   ├── Naruto T01.cbz                   → série "Naruto", tome 1,         régulier
│   └── Specials/                        ← sous-dossier spécial → remonte au parent
│       └── Naruto Special.cbz           → série "Naruto",                 hors-série
│
├── Shonen/                              ← dossier de catégorie (ignoré)
│   └── One Piece/                       ← parent immédiat = nom de série
│       ├── One Piece T01.cbz            → série "One Piece", tome 1,      régulier
│       └── One Piece T02.cbz            → série "One Piece", tome 2,      régulier
│
├── Oneshots/                            ← dossier oneshot à la racine
│   ├── Blacksad.cbz                     → série "Blacksad",  1 livre,     oneshot
│   └── Persepolis.cbz                   → série "Persepolis", 1 livre,    oneshot
│
└── livre-isole.cbz                      → pas de série (fichier à la racine)
```

**Règles de détermination de la série** :

1. La série = **répertoire parent immédiat** du fichier.
2. Si ce parent est un sous-dossier spécial (HS, Hors-Série, Specials, Bonus, Extras, Intégrales, INT…), le scanner remonte d'un cran.
3. Si le fichier est dans un dossier `Oneshots` **directement à la racine**, chaque fichier devient sa propre série.
4. Un fichier posé directement à la racine n'a pas de série.

:::note
Un dossier `Oneshots` imbriqué à 2 niveaux ou plus (`Shonen/Oneshots/…`) n'est **pas** traité comme dossier oneshot — les fichiers seront rattachés à la série `"Oneshots"`.
:::

---

## Métadonnées d'une série

Sur la page détail d'une série, vous pouvez consulter et modifier :

- **Description** de la série
- **Auteurs** (scénariste, dessinateur…)
- **Éditeurs**
- **Année de début**
- **Statut** (en cours, terminée, en pause…)
- **Nombre total de tomes** (utilisé pour calculer les volumes manquants)
- **Genres**
- **Couverture**

## Verrouillage de champs

Chaque champ peut être verrouillé individuellement pour empêcher une synchronisation de métadonnées d'écraser vos modifications manuelles. Activez le **verrou** via l'icône cadenas à côté du champ concerné.

---

## Fusionner des séries

![Modal de fusion — recherche de la série à absorber avec aperçu du nombre de livres et du provider](/screenshots/series-merge.png)

Si le scanner a créé deux séries distinctes pour ce qui est en réalité la même série (variation de nom, faute d'orthographe…), vous pouvez les fusionner :

- Ouvrez la page de la série cible
- Utilisez l'option **Fusionner avec…** pour chercher la série source à absorber
- Tous les livres de la série source sont transférés vers la série cible
- La série source est supprimée après fusion

La fusion transfère également les métadonnées, liens AniList et volumes disponibles au téléchargement. Les métadonnées de la série cible sont prioritaires en cas de conflit.

---

## Filtrer les séries

Sur la page **Séries**, plusieurs filtres sont disponibles :

**Par type de volume** — pour n'afficher que les séries contenant des tomes réguliers, des one-shots, des hors-séries ou des intégrales.

**Wishlist / bibliothèque** — les séries **sans livres** constituent votre wishlist (séries ajoutées depuis la découverte, pas encore téléchargées). Le filtre vous permet de basculer entre wishlist, séries possédées, ou tout afficher.

---

## Nettoyage automatique

Les séries qui n'ont plus aucun livre après un scan sont automatiquement supprimées — sauf si elles ont des métadonnées approuvées ou des volumes disponibles au téléchargement (c'est-à-dire les séries de votre wishlist).

:::note[Détails techniques]
**Champs de métadonnées** : `description`, `publishers`, `start_year`, `status` (`ongoing`, `ended`, `completed`, `on_hold`, `hiatus`), `total_volumes`, `authors`, `genres`, `cover_url`.

**Verrouillage** : stocké dans la colonne JSONB `locked_fields` (ex: `{"description": true}`).

**Déduplication** : `get_or_create_series` vérifie `name` et `original_name` pour éviter les doublons. Matching `LOWER(unaccent())` sur les deux champs.

**Fusion** : transfère livres, metadata links, downloads disponibles, liens AniList. Conserve les metadata links de la cible en cas de conflit (même provider). Supprime la série source après fusion.

**Filtre wishlist** : paramètre API `no_books=true` (séries sans livres) / `has_books=true` (séries avec livres). Paramètre type : `volume_type=oneshot|regular|hs|integral`.

**Nettoyage** : séries supprimées si sans livres ET sans metadata links ET sans downloads disponibles.
:::
