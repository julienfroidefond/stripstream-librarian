# Socle partagé de normalisation des titres

## Constat

Plusieurs flux transforment titres et noms de séries : import, providers de
métadonnées, statut de lecture et rapprochements internes. Ils retirent souvent
les accents, normalisent la casse et les séparateurs, mais pas toujours de la
même façon.

## Périmètre

- Ajouter dans `crates/parsers` des primitives explicites de normalisation :
  pliage des accents et clé de comparaison insensible à la casse/séparateurs.
- Faire migrer les usages dont le seul objectif est l'égalité ou la clé de
  rapprochement.
- Conserver les normalisations métier : retrait d'articles chez Bedetheque,
  score fuzzy des providers, comparaison de titres de livres dans
  `metadata/shared_sync`, et règles AniList.

## API cible

Conserver `normalize_title` pour la comparaison de mots, et ajouter seulement
si nécessaire une primitive de pliage d'accents qui préserve ponctuation et
structure. Chaque appelant choisit explicitement la représentation adaptée :
clé de comparaison ou affichage nettoyé.

## Migration

1. Cartographier les appels de normalisation et les classer : clé technique,
   scoring provider ou affichage.
2. Migrer les clés techniques, notamment le helper `strip_accents` de l'import,
   sans modifier les noms affichés ni les chemins de fichiers.
3. Ne migrer les providers que lorsque leurs tests de scoring prouvent
   l'équivalence du comportement.

## Tests et critères de fin

- Cas communs : accents français, apostrophes droites/courbes, tirets,
  underscores, espaces multiples et Unicode non latin conservé.
- Régression de recherche insensible aux accents dans l'import.
- Aucun changement de score ou d'ordre des candidats de métadonnées sans une
  décision fonctionnelle explicite.

