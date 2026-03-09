## Why

CBR est un format RAR propriétaire qui nécessite des outils tiers non-libres (`unrar`) pour la lecture. CBZ est un simple ZIP — standard ouvert, mieux supporté par les lecteurs et les outils. Permettre la conversion en backoffice évite une manipulation manuelle des fichiers et garantit une bibliothèque homogène.

## What Changes

- Nouvelle fonction `convert_cbr_to_cbz()` dans `crates/parsers`
- Nouveau endpoint API `POST /books/:id/convert` qui crée un job de conversion
- Nouveau type de job `cbr_to_cbz` traité par l'indexer
- Bouton "Convert to CBZ" sur la page détail d'un livre (visible uniquement si `file_format == 'cbr'`)
- Affichage du type `cbr_to_cbz` dans la liste des jobs du backoffice

## Capabilities

### New Capabilities

- `cbr-conversion`: Conversion d'un fichier CBR en CBZ via un job asynchrone, avec suppression sécurisée du CBR uniquement après vérification du CBZ généré.

### Modified Capabilities

<!-- Aucune spec existante n'est modifiée -->

## Impact

- **crates/parsers** : nouvelle fonction publique `convert_cbr_to_cbz`
- **apps/api** : nouveau endpoint `POST /books/:id/convert`, nouveau type de job dans `index_jobs.rs`
- **apps/indexer** : nouveau `converter.rs`, dispatch dans `worker.rs`
- **apps/backoffice** : page détail livre + liste des jobs
- **infra/migrations** : migration `0013_add_book_id_to_index_jobs.sql` pour stocker la cible du job
- **Dépendances système** : `unar` (déjà requis) + `zip` crate (déjà présent via CBZ)
