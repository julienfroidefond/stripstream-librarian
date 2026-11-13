# La navigation par genres est une requête serveur

La page `/genres` reconstruisait sa liste dans le navigateur : un `GET /series?limit=500` par couple
bibliothèque × genre, dédoublonné puis filtré par exclusion côté client (troncature silencieuse à
500, sémantique OU entre genres), une requête par genre au montage pour les couvertures et 3 × L
requêtes pour les compteurs, fusionnés par `mergeGenreCounts`. L'API n'acceptait qu'un `genre` et
un `library_id`.

`GET /genres/browse` (`apps/api/src/genres/browse.rs`, route admin) répond à toute la page en un
appel : `genres` (répétable, **ET** via `genres @> $1`), `exclude` (répétable, `NOT genres && $2`),
`library_ids` (répétable), `untagged`, `q` (`norm_text` partiel), `page`/`limit` (100 par défaut,
500 max). La réponse porte `{ items, total, page, limit, facets: { genres, untagged, total } }` ;
chaque facette de genre embarque une couverture représentative (une série avec image de préférence).
Les clés `genres[]` / `library_ids[]` sont aussi acceptées.

## Conséquences

- Les facettes ne dépendent **que** du périmètre de bibliothèques, jamais des filtres de genre, d'exclusion
  ou de nom : les puces gardent leurs compteurs pendant qu'on filtre. Le panneau « Tous les genres », les
  statistiques d'en-tête et les puces lisent la même source ; ils suivent donc désormais le filtre de
  bibliothèques (avant : panneau global, compteurs par bibliothèque).
- Côté backoffice, `browserApi.genres.browse(query)` est le seul appel de lecture de la page ;
  `lib/genres/browse.ts` (pur) traduit la sélection en requête et porte ses transitions
  (inclure/exclure, renommage, suppression). La liste est paginée (« Charger plus ») au lieu d'être tronquée.
- Les types `GenreDto`, `GenreFacet`, `GenreCover`, `GenreBrowseQuery`, `GenreBrowsePage` vivent dans
  `lib/api/genres.ts` ; la page serveur charge la première vue avec `fetchGenreBrowse`.
- `GET /genres` et `GET /genres/untagged-series` restent pour les autres consommateurs.
