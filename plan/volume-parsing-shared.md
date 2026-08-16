# Mutualiser le parsing des tomes des providers

## Constat

Google Books, OpenLibrary, Bedetheque et SensCritique extraient un numéro de
tome depuis un titre avec des regex proches (`Tome`, `T.`, `Vol`, `#`). Ces
implémentations divergent sur les séparateurs et les cas limites, ce qui rend
les résultats de métadonnées incohérents.

## Périmètre

- Créer dans `crates/parsers` une primitive pure qui extrait un tome explicite
  depuis un titre de métadonnée externe.
- Couvrir `Tome 12`, `T.12`, `Vol. 12`, `Volume 12`, `#12`, espaces et zéros
  initiaux.
- Laisser aux providers leurs règles de sélection, de groupement d'éditions et
  de fallback propres à leur API.

Ne pas utiliser ce chantier pour changer l'extraction des plages de releases
ou le matching des séries : ces besoins sont déjà couverts par `matching`.

## API cible

Exposer une fonction de type `extract_metadata_volume(title: &str) ->
Option<i32>`. Elle ne reconnaît que les marqueurs explicites ; elle ne déduit
pas un tome depuis un nombre isolé. Les providers remplacent leurs helpers
locaux par cette fonction, puis suppriment leurs regex devenues inutiles.

## Migration

1. Ajouter les tests communs dans `crates/parsers` à partir des cas existants
   des quatre providers.
2. Implémenter la primitive sans modifier les réponses des providers.
3. Migrer un provider à la fois, avec ses tests existants inchangés.
4. Retirer les helpers locaux seulement après la migration de chaque provider.

## Tests et critères de fin

- Tests unitaires communs pour les formats supportés et les faux positifs
  (`Version 2`, année, nombre sans marqueur).
- Tests provider garantissant que les `BookCandidate.volume_number` existants
  restent identiques.
- Aucun `extract_volume_number`/`extract_volume_from_title` redondant dans les
  quatre providers ciblés.

