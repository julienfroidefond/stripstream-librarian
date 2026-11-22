# Un seul modèle de lecture du catalogue des séries

Cinq surfaces renvoyaient un `SeriesItem` : `GET /series`, `GET
/libraries/{library_id}/series`, `GET /favorites`, `GET /series/ongoing` et `GET
/series/{series_id}/details`. Chacune portait sa propre requête SQL et sa propre projection des
vingt-et-une colonnes, avec trois dérives mesurables :

- une colonne ajoutée à `SeriesItem` restait `None` sur les surfaces qu'on avait oublié de mettre à
  jour — `missing_count`, `community_score` et `user_rating` n'étaient remplis que par les deux
  listes ;
- les filtres n'existaient que sur certaines surfaces (`gap`, `author`, `genre`, `volume_type`,
  `rated_only`, `no_books` et les tris `latest`/`community_score` étaient absents du lecteur par
  bibliothèque) ;
- une valeur inconnue de `sort`, `gap`, `volume_type` ou `reading_status` était silencieusement
  ignorée, et le client croyait son filtre appliqué.

`apps/api/src/series/catalog/` possède désormais la lecture. Les handlers traduisent leur DTO de
query (ou leurs paramètres de chemin) en `SeriesRequest` typé — portée, filtres, tri, pagination,
lecteur — puis appellent `catalog::query(&PgPool, SeriesRequest) -> CatalogPage`. Le module porte
la requête, la projection des lignes (`row.rs`), les filtres partagés et la restriction de genre ;
il est la seule surface publique de la lecture.

`SeriesScope` porte ce qui distingue chaque lecteur : `All { library_id }` (les deux listes et le
détail par identifiant), `Favorites` et `Ongoing`. Chaque portée fournit son préambule CTE, le
corps de son `series_counts`, son ordre (`favorited_at DESC`, `last_read_at DESC NULLS LAST`) et le
prédicat sur lequel `coverage::missing_counts_cte_for_predicate` agrège les manquants. Les filtres,
le join de progression, le shelf, la note du lecteur et la projection restent communs.

## Conséquences

- Une colonne de `SeriesItem` s'ajoute à deux endroits : la structure et la projection de `row.rs`.
  Les cinq surfaces la renvoient, sans exception.
- Une valeur invalide de `sort`, `gap`, `volume_type`, `reading_status` ou d'un drapeau est un
  `400` qui liste les valeurs acceptées. Cela vaut pour `GET /series` **et** pour `GET
  /libraries/{library_id}/series`, qui accepte désormais exactement la même query (le
  `library_id` de la query y est ignoré : le chemin fait foi).
- Les triples `missing_count`, `community_score` et `user_rating` sont désormais remplis sur les
  favoris, l'onglet « en cours » et le détail, au lieu d'être `None`.
- `GET /favorites` et `GET /series/ongoing` sans utilisateur sélectionné renvoient un `400` « no
  user selected » : un `200` avec une liste vide laissait croire qu'il n'y avait rien, alors qu'il
  n'y avait personne.
- La forme publique ne change pas : les listes renvoient `SeriesPage`, les favoris et l'onglet « en
  cours » un tableau nu de `SeriesItem`, et le détail un seul `SeriesItem`. Le client de lecture
  n'a rien à adapter.
- `coverage::missing_counts_cte_for_predicate` remplace les variantes par bibliothèque et par
  série : une portée énonce son prédicat (`s.library_id = $1`, `s.id = $1`, une appartenance
  ensembliste), la règle de couverture reste dans `stripstream_core::coverage`.
- Un test différentiel appelle les deux listes avec les mêmes filtres et compare les réponses
  sérialisées : toute divergence entre `GET /series?library_id=` et `GET
  /libraries/{id}/series` casse la suite.
- Deux surfaces de genre (`GET /genres/browse`, `GET /genres/untagged-series`) réutilisent encore
  `SeriesItem` comme support d'affichage avec leur propre requête, sans `missing_count`,
  `community_score` ni `user_rating`. Elles restent hors de ce modèle : une nouvelle colonne
  affichée sur ces cartes devra y être ajoutée à la main tant qu'elles n'auront pas leur propre
  type de vue.
