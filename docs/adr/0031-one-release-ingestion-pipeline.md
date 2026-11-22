# Un seul point de publication des releases

Trois flux écrivaient `available_downloads` : la détection Prowlarr par série
(`downloads/detection.rs`), le poll RSS Prowlarr global (`downloads/rss_poll.rs`) et la recherche
DDL (`downloads/ddl_provider/search.rs`). Chacun collecte différemment — une recherche par série,
un fetch global mis en correspondance en mémoire, un catalogue par fournisseur — mais chacun
répétait la même publication : filtrer la liste noire, horodater `detected_at`, émettre
l'événement « trouvé » du job, fusionner via `availability::merge_releases`, compter les releases
nouvelles et nettoyer les lignes vidées.

Les copies avaient divergé sur onze points, sans qu'aucun ne soit une décision écrite :

| Étape | Détection | RSS | DDL |
|---|---|---|---|
| Périmètre | toutes les séries de la bibliothèque | toutes les séries du lien | tous les liens avec manquants, toutes bibliothèques |
| Liste noire | après collecte, avec `all_blacklisted` | pendant la mise en correspondance | dans l'entonnoir, comptée `files_blacklisted` |
| Échec de fusion | ignoré (`let _ =`) | ignoré (`let _ =`) | remonté (`?`, job en erreur) |
| Événement | `downloads_found`, releases incluses | `downloads_found`, releases incluses | `ddl_downloads_found`, entité `series`, compteurs seuls |
| Nouveaux depuis le job | SQL bibliothèque | SQL bibliothèque + global | aucun (compteurs de fichiers) |
| Nettoyage | `normalize_library` + `delete_empty(lib)` | aucun | `delete_empty(global)` |

`apps/api/src/downloads/ingest.rs` possède désormais la publication. `store_releases` filtre la
liste noire, pose `detected_at` (une seule fois : `merge_releases` conserve la valeur d'une release
déjà connue), émet l'événement, fusionne et renvoie `Stored::Nothing` ou `Stored::Found(n)`. Ce qui
distinguait les flux est devenu des données : `StorePolicy` (nom d'événement, `EntityKind`,
`ReleaseDetail`, `MergeFailure`) avec un constructeur par source (`prowlarr_detection`,
`prowlarr_rss`, `ddl_catalogue`), et `Cleanup` (`Keep`, `DeleteEmpty`, `NormalizeThenDeleteEmpty`).
`new_releases_since`, `new_items_since` et `event_counts` sont les trois lectures que les bilans de
job recopiaient.

Les boucles de collecte restent dans leurs modules : c'est là que les trois sources sont
irréductiblement différentes (appels Prowlarr, mise en correspondance RSS, entonnoir et dédup DDL).
Le type `AvailableReleaseDto` vit dans `downloads/release.rs`, le contrat JSON commun aux trois
sources, et non dans `detection`.

## Conséquences

- Une release entre dans `available_downloads` par `store_releases` uniquement. Un nouveau flux
  (autre indexeur, autre catalogue) fournit ses releases collectées et choisit un `StorePolicy`
  et un `Cleanup` ; il n'écrit ni l'événement, ni l'horodatage, ni la fusion.
- Une différence entre deux flux est un choix visible dans un constructeur de `StorePolicy`,
  jamais un `let _ =` recopié. Changer une politique change les trois flux de la même façon.
- Le vocabulaire d'événements est inchangé : `downloads_found`, `downloads_not_found`,
  `ddl_downloads_found`, `prowlarr_no_results`, `no_missing_volumes`, `no_metadata_link` restent
  ceux documentés dans `jobs/telechargements.md` ; les flux décident seulement du moment où ils
  émettent.
- La détection émet toujours `downloads_found` avant la fusion, et le détail DDL reste sans
  `available_releases` : les rapports de job et le backoffice n'ont rien à adapter.
- Un échec de fusion ignoré est maintenant journalisé (`warn!`) au lieu d'être silencieux ; il ne
  change pas le statut du job.
- `merge_releases` reste le seul propriétaire de la fusion et du compteur (ADR 0006) ; `ingest`
  ne réimplémente aucune règle de volume.
- Le polling RSS ne nettoie pas : la détection suivante enlève les lignes dont la série est
  complète. C'est la politique `Cleanup::Keep`, écrite au point d'appel.
