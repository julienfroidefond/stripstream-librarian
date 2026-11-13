# Un seul get-or-create de série : `series_identity::find_or_create`

Trois chemins créaient une série à partir d'un nom brut : le scanner (`get_or_create_series_id`), l'API (`series::helpers::get_or_create_series`, utilisé par la création de série, Discovery et le rattachement de métadonnées) et l'édition d'un livre (`PUT /books/{id}`). Les deux premiers réécrivaient à la main `norm_text(name) OR norm_text(original_name)`, contre l'ADR 0007 ; le troisième faisait `INSERT … ON CONFLICT (library_id, name)` sans normalisation et créait « one piece » à côté de « One Piece ». `series_identity::find` ignorait `original_name` : la règle d'identité dépendait du chemin.

`crates/core/src/series_identity.rs` expose désormais `find_or_create(conn, library_id, raw) -> SeriesIdentity`, seul get-or-create de série. Il accepte un pool, une connexion ou une transaction.

## Règle d'identité

Un nom brut désigne la série de la bibliothèque dont `norm_text(name)` **ou** `norm_text(original_name)` vaut `norm_text(raw)`. `original_name` conserve le nom de dossier après un renommage utilisateur : le scanner, l'API et les téléchargements continuent de rattacher l'ancien nom à la série renommée au lieu de la recréer. Si plusieurs lignes correspondent, une correspondance sur `name` l'emporte sur `original_name`, puis la plus ancienne. `find`, `find_id`, `find_many`, `resolve_many` et `existing_names` appliquent la même règle.

## Concurrence

La contrainte unique ne porte que sur `(library_id, name)` exact et l'index `norm_text` n'est pas unique. Plutôt qu'une migration (fusion des doublons existants puis index unique sur `norm_text`), `find_or_create` prend un verrou consultatif transactionnel sur `(library_id, norm_text(raw))`, refait la recherche sous le verrou, puis insère. Deux créations simultanées de « One Piece » et « one piece » donnent une seule série. Le `ON CONFLICT (library_id, name)` reste un filet pour les écrivains hors module.

## Conséquences

- Pas de migration : les doublons normalisés déjà en base ne sont pas fusionnés ; la recherche choisit une ligne de façon déterministe.
- Le cache du scanner devient un mémo par nom brut exact ; la normalisation reste en SQL.
- `find_or_create_tracked` renvoie en plus `created` : le scanner ne compte comme nouvelle série (notifications) que celles réellement insérées, jamais une autre graphie d'une série existante.
- Les tests de get-or-create dupliqués côté indexer et API sont supprimés ; ils vivent dans le crate `core` (casse/accents, `original_name`, renommage en chaîne, transaction, concurrence).
- Reste hors module : `metadata::field_policy::apply_series` fait encore un upsert sur le nom exact ; il ne s'applique qu'à une série déjà résolue.
