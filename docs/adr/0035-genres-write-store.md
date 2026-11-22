# Un store d'écriture unique pour les genres

L'atelier de genres (`apps/backoffice/app/(app)/genres/GenresClient.tsx`) écrit quatre fois :
renommer un genre, le supprimer, assigner un genre à une sélection de séries, appliquer un tag
suggéré par l'IA. Les quatre rejouaient le même squelette `setBusy(true)` / `try` / relecture /
toast / `finally`, et partageaient un booléen `busy` global : renommer un genre bloquait
l'assignation d'un autre, le bouton « Suggest » (une lecture) attendait les écritures, et les
règles de concurrence n'étaient couvertes par aucun test unitaire. Le seul parcours E2E qui les
touchait était périmé : il attendait `/api/genres/untagged-series` et `/api/series?genre=`, que
la page n'appelle plus depuis l'[ADR 0028](./0028-genres-browse-query.md).

`lib/genres/actions.ts` (pur, sans React) porte désormais le protocole :

- un vocabulaire d'écritures (`GenreWrite`) routé vers `browserApi.genres` : `rename`, `delete`,
  `assign`, et `applySuggestion` (même route que `assign`, le discriminant est conservé pour que
  le message d'erreur de l'IA reste distinct) ;
- le **sujet** d'une écriture est le nom du genre, parce que l'API écrit par nom : deux écritures
  sur des genres différents partent en parallèle, deux écritures sur le même genre sont
  sérialisées, et une écriture identique (même verbe, mêmes arguments, identifiants de séries
  triés) est coalescée sur la requête en vol — une rafale de clics ne produit qu'un appel ;
- `busy(genre)` est vrai dès l'appel et jusqu'au règlement de **toutes** les écritures du sujet,
  continuation d'après-écriture comprise ;
- une seule politique d'échec : l'écriture se règle sur `false`, `onError(error, write)` remonte
  au toast, la continuation n'est pas exécutée et l'écriture suivante du même sujet s'exécute
  quand même. Aucune relecture n'est déclenchée par le store.

Le store ne connaît pas le read model. `run(write, after?)` reçoit la continuation de la page
(transition de sélection via `lib/genres/browse.ts`, relecture `/genres/browse`, toast de succès)
et la garde dans la fenêtre busy : la modale et les boutons du genre ne se réarment qu'une fois
la liste réconciliée. La continuation ne peut pas transformer une écriture acceptée en échec ;
ses propres erreurs de lecture sont déjà traitées par la page.

`app/(app)/genres/useGenreActions.ts` monte le store pour la page (`useSyncExternalStore`) et
branche `onError` sur le toast existant (`genres.aiError` pour une suggestion, `common.error`
sinon). `GenresClient` ne garde aucun booléen `busy` global et n'appelle plus
`browserApi.genres.rename/remove/assign` : il déclenche `run(write, after)` et lit `busy(genre)`
pour désactiver les contrôles du seul genre concerné. La sélection courante est lue par référence
pour qu'une relecture tardive n'écrase pas une transition plus récente.

## Conséquences

- Renommer un genre ne bloque plus l'assignation d'un autre ; les contrôles d'un genre ne sont
  désactivés que par une écriture sur ce genre, et le bouton « Suggest » n'attend plus les
  écritures.
- Les règles de concurrence, la coalescence, la sérialisation et les deux branches d'échec sont
  couvertes par `tests/unit/lib/genres/actions.test.ts` ; le spec E2E
  `catalog/genres-actions.spec.ts` ne couvre plus que les parcours visibles (assignation, tag IA,
  renommage, suppression) et attend le contrat `/genres/browse` de l'ADR 0028.
- `GET /genres/untagged-series` reste pour les autres consommateurs ; la page ne l'appelle plus.
- ADR 0028 reste le contrat de lecture : ce store ne fait qu'écrire et ne construit jamais de
  `SeriesItem`.
