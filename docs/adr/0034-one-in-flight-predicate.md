# Un seul prédicat pour « un job est en vol »

`JobState::IN_FLIGHT` (pending, running, `extracting_pages`, `generating_thumbnails`) est la
définition partagée de « la tâche occupe encore son créneau ». Trois formes coexistaient :

- `stripstream_jobs::job_in_flight(scope, &[types])`, testé sur les scopes, consommé par le
  lanceur de l'API, les déclencheurs Telegram et DDL ;
- `apps/indexer/src/scheduler.rs::in_flight_statuses()`, qui dérivait de `JobState::IN_FLIGHT`
  mais restait local ;
- `apps/indexer/src/watcher.rs`, qui recopiait les quatre statuts **en littéral**, à deux endroits.

Le watcher ne pouvait pas appeler le seam : `job_in_flight` exigeait une liste de types, et
« n'importe quel type » n'était pas représentable. Le prédicat testé existait donc sans son
consommateur, et la seule copie littérale des statuts était celle qui n'était couverte par rien.

## Décisions

- **`JobTypes::{Any, Only}`** : `JobTypes::Any` est la règle « tout job en vol me bloque » ;
  `JobTypes::Only(&[...])` restreint aux types en conflit. Le filtre de type disparaît du SQL
  quand la liste est vide, donc l'appelant n'a jamais à passer un slice vide ambigu.
- **Le watcher consomme le seam** : `any_job_in_flight` interroge `JobScope::Any`,
  `library_is_busy` interroge `JobScope::Library(id)`, tous deux en `JobTypes::Any`. Ce sont les
  deux seules copies du prédicat, et elles passent par les tests de `lifecycle`.
- **Les politiques d'erreur restent distinctes et deviennent nommées** : un échec de
  `any_job_in_flight` laisse le poll continuer (le snapshot est bon marché, le poll suivant
  réessaie) ; un échec de `library_is_busy` est traité comme « occupé » pour ne jamais créer un
  rebuild en double. Les deux erreurs sont désormais loggées, au lieu d'être avalées par
  `unwrap_or`.
- **Le scheduler garde ses requêtes ensemblistes** : elles portent les portes de configuration,
  les prédicats métier (`anilist_series_links`, `external_metadata_links`), le type dérivé de
  `scan_mode`, le stamping des colonnes `next_*` et les trois variantes globales avec
  `recent_run`. Elles bindent `JobState::in_flight_strings()` ; le helper local est supprimé.
  Une table de familles aurait transformé sept politiques en fragments SQL et n'aurait économisé
  que la squelette « dû → pas en vol → insert → stamp ».
- **ADR 0002 n'est pas rouvert** : la création reste dans le service qui planifie la tâche ;
  seul le prédicat de conflit est partagé.

## Conséquences

- Les quatre statuts en vol n'existent plus en littéral hors de `JobState` : ajouter un statut
  in-flight met à jour le watcher, le scheduler, le lanceur et les tests d'un coup.
- `job_in_flight` est couvert par un cas `JobTypes::Any` en plus des scopes et des types.
- Le watcher est couvert par `#[sqlx::test]` : chaque phase active bloque le poll, une
  bibliothèque occupée ne bloque que son propre rebuild, un job terminal libère le créneau.
- Comportement visible inchangé : mêmes décisions de poll et d'insertion, seuls les messages
  d'erreur SQL apparaissent désormais dans les logs.
