# L'identité d'une série est la clé `norm_text(name)`

Les lookups de séries utilisaient des comparaisons différentes en Rust et en SQL, avec un risque de divergence sur la casse et les accents. `crates/core/src/series_identity.rs` centralise les lookups de résolution autour de la clé SQL `norm_text(name)` (minuscules et `unaccent`), adossée à l'index `(library_id, norm_text(name))` : les appelants fournissent le nom brut et la base normalise les deux côtés.

## Conséquences

- `find`, `find_id` et `find_many` sont limités à une bibliothèque ; `resolve_many` traite un lot de couples `(library_id, nom brut)` et `existing_names` filtre un lot de noms contre toutes les bibliothèques.
- Les requêtes set-based qui joignent déjà `norm_text` à d'autres tables (recherche, résolution de dossier d'import) restent en place ; un nouveau lookup unitaire ou batch passe par ce module, jamais par un `LOWER(name)` ou un `norm_text` écrit à la main.
